//! LogManager —— 日志系统管理
//!
//! 内存环形缓冲（上限 2000 条）+ 按本地日期分文件持久化 + 过滤查询 + 导出/导入。
//! 每条日志同时经 `EventBus` 发布，驱动管理端 WS 实时推送。日志清空仅清内存
//! （不清磁盘文件）；旧日志清理由 `cleanup_old` 负责（F-3，按 `logRetentionDays` 删除过期文件）。

use std::path::PathBuf;
use std::sync::Mutex;

use crate::backend::constants::*;
use crate::backend::eventbus::EventBus;
use crate::backend::types::*;

/// 日志等级过滤顺序
const LEVEL_ORDER: [&str; 4] = ["DEBUG", "INFO", "WARN", "ERROR"];

/// 日志管理器
pub struct LogManager {
    entries: Mutex<Vec<LogEntry>>,
    max_memory: usize,
    log_dir: PathBuf,
    event_bus: EventBus,
}

impl LogManager {
    pub fn new(log_dir: PathBuf, event_bus: EventBus) -> Self {
        let _ = std::fs::create_dir_all(&log_dir);
        Self {
            entries: Mutex::new(Vec::new()),
            max_memory: LOG_MEMORY_LIMIT,
            log_dir,
            event_bus,
        }
    }

    /// 添加一条日志（写内存 + 落盘 + 发布到 EventBus）
    pub fn add_entry(&self, mut entry: LogEntry) {
        if entry.id.is_empty() {
            entry.id = uuid::Uuid::new_v4().to_string();
        }
        if entry.timestamp.is_empty() {
            entry.timestamp = now_rfc3339();
        }

        {
            let mut g = self.entries.lock().unwrap();
            g.push(entry.clone());
            if g.len() > self.max_memory {
                let overflow = g.len() - self.max_memory;
                g.drain(0..overflow);
            }
        }

        self.write_to_file(&entry);
        self.event_bus.publish_log(entry);
    }

    /// 查询（支持 serverId / level 下限 / keyword 过滤）
    pub fn get_entries(&self, filter: &LogFilter) -> Vec<LogEntry> {
        let g = self.entries.lock().unwrap();
        let mut result: Vec<LogEntry> = g.iter().cloned().collect();

        if let Some(sid) = &filter.server_id {
            result.retain(|e| e.server_id.as_deref() == Some(sid.as_str()));
        }
        if let Some(level) = &filter.level {
            let min_idx = LEVEL_ORDER
                .iter()
                .position(|l| *l == level.as_str())
                .unwrap_or(0);
            result.retain(|e| {
                LEVEL_ORDER
                    .iter()
                    .position(|l| *l == e.level.as_str())
                    .map(|idx| idx >= min_idx)
                    .unwrap_or(false)
            });
        }
        if let Some(kw) = &filter.keyword {
            let kw = kw.to_lowercase();
            result.retain(|e| {
                e.message.to_lowercase().contains(&kw)
                    || e.server_id
                        .as_ref()
                        .map(|s| s.to_lowercase().contains(&kw))
                        .unwrap_or(false)
            });
        }
        result
    }

    /// 取最近 n 条（用于初始 `log_batch`）
    pub fn get_entries_last(&self, n: usize) -> Vec<LogEntry> {
        let g = self.entries.lock().unwrap();
        if g.len() <= n {
            g.clone()
        } else {
            g[g.len() - n..].to_vec()
        }
    }

    /// 清空内存日志（不影响磁盘文件）
    pub fn clear_entries(&self) {
        self.entries.lock().unwrap().clear();
    }

    /// 按保留天数清理磁盘日志文件（F-3）。
    ///
    /// 仅删除 `log_dir` 下文件名形如 `YYYY-MM-DD.log` 且日期早于
    /// `今天 - retention_days` 的文件；其他文件（如非日期命名的 `.log`）不受影响。
    /// 返回被删除的文件数量，便于日志/测试观测。
    pub fn cleanup_old(&self, retention_days: u64) -> usize {
        let today = chrono::Local::now().date_naive();
        let cutoff = match today.checked_sub_signed(chrono::Duration::days(retention_days as i64)) {
            Some(d) => d,
            None => return 0,
        };
        let mut removed = 0usize;
        if let Ok(entries) = std::fs::read_dir(&self.log_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                // 仅处理 `.log` 文件
                if path.extension().and_then(|e| e.to_str()) != Some("log") {
                    continue;
                }
                // 文件名（去扩展名）须为 `YYYY-MM-DD`
                let stem = match path.file_stem().and_then(|s| s.to_str()) {
                    Some(s) => s,
                    None => continue,
                };
                if let Ok(file_date) = chrono::NaiveDate::parse_from_str(stem, "%Y-%m-%d") {
                    if file_date < cutoff {
                        if std::fs::remove_file(&path).is_ok() {
                            removed += 1;
                        }
                    }
                }
            }
        }
        removed
    }

    // ==================== 内部方法 ====================

    fn write_to_file(&self, entry: &LogEntry) {
        let path = self.log_dir.join(format!("{}.log", local_date_string()));
        let line = match serde_json::to_string(entry) {
            Ok(s) => s + "\n",
            Err(_) => return,
        };
        // 异步落盘，避免阻塞调用方
        let _ = tauri::async_runtime::spawn_blocking(move || {
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
            {
                let _ = f.write_all(line.as_bytes());
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::LogManager;
    use crate::backend::eventbus::EventBus;
    use crate::backend::types::*;

    fn tmp_log_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ssm_log_{}", uuid::Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    fn mk_entry(server_id: Option<&str>, level: LogLevel, msg: &str) -> LogEntry {
        LogEntry {
            server_id: server_id.map(|s| s.to_string()),
            level,
            event: "test".to_string(),
            message: msg.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn add_entry_then_log_batch_returns_it() {
        let m = LogManager::new(tmp_log_dir(), EventBus::new());
        m.add_entry(mk_entry(Some("s1"), LogLevel::Info, "hello"));
        let batch = m.get_entries_last(10);
        assert_eq!(batch.len(), 1, "log_batch should include the new entry");
        assert_eq!(batch[0].message, "hello");
        assert_eq!(batch[0].server_id.as_deref(), Some("s1"));
        assert!(!batch[0].id.is_empty(), "id should be auto-filled");
        assert_eq!(m.get_entries(&LogFilter::default()).len(), 1);
    }

    #[test]
    fn clear_empties_buffer() {
        let m = LogManager::new(tmp_log_dir(), EventBus::new());
        m.add_entry(mk_entry(Some("s1"), LogLevel::Info, "a"));
        m.add_entry(mk_entry(Some("s1"), LogLevel::Warn, "b"));
        assert_eq!(m.get_entries_last(10).len(), 2);
        m.clear_entries();
        assert_eq!(m.get_entries_last(10).len(), 0);
        assert_eq!(m.get_entries(&LogFilter::default()).len(), 0);
    }

    #[test]
    fn filter_by_server_and_level() {
        let m = LogManager::new(tmp_log_dir(), EventBus::new());
        m.add_entry(mk_entry(Some("s1"), LogLevel::Info, "info-1"));
        m.add_entry(mk_entry(Some("s1"), LogLevel::Error, "err-1"));
        m.add_entry(mk_entry(Some("s2"), LogLevel::Info, "info-2"));
        let s1 = m.get_entries(&LogFilter {
            server_id: Some("s1".to_string()),
            ..Default::default()
        });
        assert_eq!(s1.len(), 2);
        let warn = m.get_entries(&LogFilter {
            level: Some(LogLevel::Warn),
            ..Default::default()
        });
        assert_eq!(warn.len(), 1);
        assert_eq!(warn[0].message, "err-1");
    }

    #[test]
    fn cleanup_old_removes_only_expired_files() {
        let dir = tmp_log_dir();
        let m = LogManager::new(dir.clone(), EventBus::new());

        let today = chrono::Local::now().date_naive();
        let old = today - chrono::Duration::days(30);
        let recent = today - chrono::Duration::days(2);
        let make = |d: chrono::NaiveDate| {
            let p = dir.join(format!("{}.log", d.format("%Y-%m-%d")));
            std::fs::write(&p, "{}").unwrap();
            p
        };
        let old_p = make(old);
        let recent_p = make(recent);
        let other_p = dir.join("notadate.log");
        std::fs::write(&other_p, "x").unwrap();

        // 保留 7 天：30 天前的文件应删，2 天前的与非日期命名文件应保留
        let removed = m.cleanup_old(7);
        assert_eq!(removed, 1, "只有 30 天前的文件应被删除");
        assert!(!old_p.exists(), "过期文件应已删除");
        assert!(recent_p.exists(), "2 天前的文件应保留");
        assert!(other_p.exists(), "非日期命名的 .log 不应被删");

        let _ = std::fs::remove_dir_all(&dir);
    }
}

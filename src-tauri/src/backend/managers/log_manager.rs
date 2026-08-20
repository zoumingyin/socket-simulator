//! LogManager —— 日志系统管理
//!
//! 内存环形缓冲（上限 2000 条）+ 按本地日期分文件持久化 + SQLite 持久化查询（P1-4）
//! + 过滤查询 + 导出/导入。
//! 每条日志同时经 `EventBus` 发布，驱动管理端 WS 实时推送。日志清空仅清内存
//! （不清磁盘文件）；旧日志清理由 `cleanup_old` 负责（F-3，按 `logRetentionDays` 删除过期文件）。
//!
//! P1-4 增量：`logs.db`（SQLite）持久化全部日志，`query_persisted` 支持跨重启的
//! 历史分页过滤查询（REST：`GET /api/logs/persisted`）。内存/文件/SQLite 三路写入，
//! SQLite 打开失败时静默降级（不影响既有内存+文件行为）。

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

use crate::backend::constants::*;
use crate::backend::eventbus::EventBus;
use crate::backend::types::*;

/// 日志等级过滤顺序
const LEVEL_ORDER: [&str; 4] = ["DEBUG", "INFO", "WARN", "ERROR"];

/// 等级 → 数值（用于 SQL 下限过滤）
fn level_num(level: &str) -> i64 {
    LEVEL_ORDER.iter().position(|l| *l == level).unwrap_or(0) as i64
}

/// SQLite 持久化存储（P1-4）：专用 DB 线程 + 独立 multi_thread runtime + 消息通道。
///
/// 写入（fire-and-forget）与查询（同步等待回执）均经 `std::sync::mpsc` 发往 DB 线程，
/// 由该线程在自己的 runtime 内执行 SQL —— 完全绕开调用方所在线程的 tokio runtime
/// （避免「在 runtime 内创建 runtime」panic，兼容 `#[tokio::test]` 与 Tauri 主线程）。
/// 打开/建表失败时返回 `None`（调用方静默降级为内存+文件模式）。
struct LogDb {
    tx: std::sync::mpsc::Sender<DbMsg>,
}

enum DbMsg {
    Insert(LogEntry),
    Query {
        filter: LogFilter,
        limit: i64,
        offset: i64,
        reply: std::sync::mpsc::Sender<(i64, Vec<LogEntry>)>,
    },
    /// 预留：优雅关闭信号（当前 LogDb 线程随进程退出自然结束，未接线）
    #[allow(dead_code)]
    Shutdown,
}

impl LogDb {
    fn new_blocking(data_dir: &Path) -> Option<Self> {
        let dir = data_dir.to_path_buf();
        let (tx, rx) = std::sync::mpsc::channel::<DbMsg>();
        let (init_tx, init_rx) = std::sync::mpsc::channel::<()>();

        std::thread::spawn(move || {
            // 专用线程内创建 runtime（无外部 runtime 上下文）
            let rt = match tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(_) => return,
            };
            let db_path = dir.join("logs.db");
            let opts = SqliteConnectOptions::new()
                .filename(&db_path)
                .create_if_missing(true);
            let pool = match rt.block_on(async {
                SqlitePoolOptions::new()
                    .max_connections(1)
                    .connect_with(opts)
                    .await
            }) {
                Ok(p) => p,
                Err(_) => return,
            };
            if rt.block_on(Self::init_schema(&pool)).is_err() {
                return;
            }
            let _ = init_tx.send(());

            loop {
                match rx.recv() {
                    Ok(DbMsg::Insert(e)) => {
                        Self::insert_blocking(&rt, &pool, &e);
                    }
                    Ok(DbMsg::Query {
                        filter,
                        limit,
                        offset,
                        reply,
                    }) => {
                        let (total, items) =
                            Self::query_blocking(&rt, &pool, &filter, limit, offset);
                        let _ = reply.send((total, items));
                    }
                    Ok(DbMsg::Shutdown) | Err(_) => break,
                }
            }
        });

        // 等待初始化完成（带超时，避免死等）
        let _ = init_rx.recv_timeout(std::time::Duration::from_secs(10));
        Some(Self { tx })
    }

    /// 写入（fire-and-forget：不阻塞调用方）
    fn insert(&self, e: &LogEntry) {
        let _ = self.tx.send(DbMsg::Insert(e.clone()));
    }

    /// 历史分页过滤查询（同步等待 DB 线程回执）
    fn query_persisted(&self, filter: &LogFilter, limit: i64, offset: i64) -> (i64, Vec<LogEntry>) {
        let (reply_tx, reply_rx) = std::sync::mpsc::channel();
        let msg = DbMsg::Query {
            filter: filter.clone(),
            limit,
            offset,
            reply: reply_tx,
        };
        if self.tx.send(msg).is_err() {
            return (0, Vec::new());
        }
        reply_rx.recv().unwrap_or((0, Vec::new()))
    }

    async fn init_schema(pool: &SqlitePool) -> Result<(), sqlx::Error> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS log_entries (
                id TEXT PRIMARY KEY,
                timestamp TEXT NOT NULL,
                server_id TEXT,
                level TEXT NOT NULL,
                event TEXT NOT NULL,
                message TEXT NOT NULL,
                client_id TEXT,
                metadata TEXT
            )",
        )
        .execute(pool)
        .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_log_timestamp ON log_entries (timestamp DESC)")
            .execute(pool)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_log_server ON log_entries (server_id)")
            .execute(pool)
            .await?;
        Ok(())
    }

    fn insert_blocking(rt: &tokio::runtime::Runtime, pool: &SqlitePool, e: &LogEntry) {
        let pool = pool.clone();
        let id = e.id.clone();
        let ts = e.timestamp.clone();
        let sid = e.server_id.clone();
        let level = e.level.as_str().to_string();
        let event = e.event.clone();
        let message = e.message.clone();
        let cid = e.client_id.clone();
        let meta = e.metadata.as_ref().map(|v| v.to_string());
        let _ = rt.block_on(async move {
            sqlx::query(
                "INSERT OR REPLACE INTO log_entries
                    (id, timestamp, server_id, level, event, message, client_id, metadata)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&id)
            .bind(&ts)
            .bind(&sid)
            .bind(&level)
            .bind(&event)
            .bind(&message)
            .bind(&cid)
            .bind(&meta)
            .execute(&pool)
            .await
        });
    }

    /// 历史分页过滤查询：返回 `(total, items)`（时间倒序）
    fn query_blocking(
        rt: &tokio::runtime::Runtime,
        pool: &SqlitePool,
        filter: &LogFilter,
        limit: i64,
        offset: i64,
    ) -> (i64, Vec<LogEntry>) {
        let pool = pool.clone();
        let sid = filter.server_id.clone();
        let level = filter.level.map(|l| level_num(l.as_str()));
        let kw = filter.keyword.as_ref().map(|k| k.to_lowercase());
        rt.block_on(async move {
            let mut where_sql = String::new();
            let mut conditions = Vec::new();
            if let Some(_s) = &sid {
                conditions.push("server_id = ?");
            }
            if level.is_some() {
                conditions.push(
                    "CASE level WHEN 'DEBUG' THEN 0 WHEN 'INFO' THEN 1 WHEN 'WARN' THEN 2 WHEN 'ERROR' THEN 3 ELSE 0 END >= ?",
                );
            }
            if kw.is_some() {
                conditions.push("(instr(lower(message), ?) > 0 OR instr(lower(coalesce(server_id, '')), ?) > 0)");
            }
            if !conditions.is_empty() {
                where_sql = format!(" WHERE {}", conditions.join(" AND "));
            }

            let count_sql = format!("SELECT COUNT(*) FROM log_entries{}", where_sql);
            let mut cq = sqlx::query(&count_sql);
            if let Some(s) = &sid {
                cq = cq.bind(s);
            }
            if let Some(lv) = level {
                cq = cq.bind(lv);
            }
            if let Some(k) = &kw {
                cq = cq.bind(k).bind(k);
            }
            let total: i64 = cq.fetch_one(&pool).await.map(|r| r.get(0)).unwrap_or(0);

            let list_sql = format!(
                "SELECT id, timestamp, server_id, level, event, message, client_id, metadata
                 FROM log_entries{} ORDER BY timestamp DESC, id DESC LIMIT ? OFFSET ?",
                where_sql
            );
            let mut lq = sqlx::query(&list_sql);
            if let Some(s) = &sid {
                lq = lq.bind(s);
            }
            if let Some(lv) = level {
                lq = lq.bind(lv);
            }
            if let Some(k) = &kw {
                lq = lq.bind(k).bind(k);
            }
            let rows = lq.bind(limit).bind(offset).fetch_all(&pool).await;
            let items = match rows {
                Ok(rows) => rows
                    .into_iter()
                    .map(|row| LogEntry {
                        id: row.get(0),
                        timestamp: row.get(1),
                        server_id: row.get(2),
                        level: serde_json::from_value::<LogLevel>(serde_json::Value::String(
                            row.get::<String, _>(3),
                        ))
                        .unwrap_or(LogLevel::Info),
                        event: row.get(4),
                        message: row.get(5),
                        client_id: row.get(6),
                        metadata: row
                            .get::<Option<String>, _>(7)
                            .and_then(|s| serde_json::from_str(&s).ok()),
                    })
                    .collect(),
                Err(_) => Vec::new(),
            };
            (total, items)
        })
    }
}

/// 日志管理器
pub struct LogManager {
    entries: Mutex<Vec<LogEntry>>,
    max_memory: usize,
    log_dir: PathBuf,
    event_bus: EventBus,
    db: Option<LogDb>,
}

impl LogManager {
    pub fn new(log_dir: PathBuf, event_bus: EventBus) -> Self {
        let _ = std::fs::create_dir_all(&log_dir);
        // P1-4：SQLite 持久化（失败静默降级）
        let db = LogDb::new_blocking(&log_dir);
        if db.is_none() {
            eprintln!("[LogManager] SQLite 持久化初始化失败，降级为内存+文件模式");
        }
        Self {
            entries: Mutex::new(Vec::new()),
            max_memory: LOG_MEMORY_LIMIT,
            log_dir,
            event_bus,
            db,
        }
    }

    /// 添加一条日志（写内存 + 落盘 + SQLite + 发布到 EventBus）
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
        if let Some(db) = &self.db {
            db.insert(&entry);
        }
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

    /// 历史持久化分页查询（P1-4）：从 SQLite 读取，支持 serverId/level 下限/keyword 过滤。
    /// 返回 `(total, items)`；SQLite 不可用时返回 `(0, [])`（不 panic）。
    pub fn query_persisted(&self, filter: &LogFilter, limit: i64, offset: i64) -> (i64, Vec<LogEntry>) {
        match &self.db {
            Some(db) => db.query_persisted(filter, limit, offset),
            None => (0, Vec::new()),
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

    // ==================== P1-4 SQLite 持久化 ====================

    #[test]
    fn persisted_roundtrip_and_filter() {
        let m = LogManager::new(tmp_log_dir(), EventBus::new());
        assert!(m.db.is_some(), "SQLite 持久化应初始化成功");

        m.add_entry(mk_entry(Some("s1"), LogLevel::Info, "info-1"));
        m.add_entry(mk_entry(Some("s1"), LogLevel::Error, "err-1"));
        m.add_entry(mk_entry(Some("s2"), LogLevel::Debug, "dbg-2"));

        // 全量（时间倒序）
        let (total, items) = m.query_persisted(&LogFilter::default(), 100, 0);
        assert_eq!(total, 3, "SQLite 应持久化全部 3 条");
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].message, "dbg-2", "时间倒序（同秒按 id 倒序，最后写入在前）");
        assert_eq!(items[0].level, LogLevel::Debug);

        // 按服务过滤
        let (total, items) = m.query_persisted(
            &LogFilter {
                server_id: Some("s1".to_string()),
                ..Default::default()
            },
            100,
            0,
        );
        assert_eq!(total, 2);
        assert!(items.iter().all(|e| e.server_id.as_deref() == Some("s1")));

        // 等级下限：WARN 起（含 ERROR）
        let (total, items) = m.query_persisted(
            &LogFilter {
                level: Some(LogLevel::Warn),
                ..Default::default()
            },
            100,
            0,
        );
        assert_eq!(total, 1);
        assert_eq!(items[0].message, "err-1");

        // 关键词
        let (total, items) = m.query_persisted(
            &LogFilter {
                keyword: Some("info".to_string()),
                ..Default::default()
            },
            100,
            0,
        );
        assert_eq!(total, 1);
        assert_eq!(items[0].message, "info-1");

        // 分页：limit=2 offset=0 → 2 条；offset=2 → 1 条
        let (total, items) = m.query_persisted(&LogFilter::default(), 2, 0);
        assert_eq!(total, 3);
        assert_eq!(items.len(), 2);
        let (_, items2) = m.query_persisted(&LogFilter::default(), 2, 2);
        assert_eq!(items2.len(), 1);
    }

    #[test]
    fn persisted_survives_recreation() {
        // 新管理器指向同一目录 → 历史仍在（跨重启可查）
        let dir = tmp_log_dir();
        {
            let m = LogManager::new(dir.clone(), EventBus::new());
            m.add_entry(mk_entry(Some("s9"), LogLevel::Warn, "persist-me"));
        }
        let m2 = LogManager::new(dir.clone(), EventBus::new());
        let (total, items) = m2.query_persisted(&LogFilter::default(), 10, 0);
        assert_eq!(total, 1, "重建管理器后历史应可查");
        assert_eq!(items[0].message, "persist-me");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
//! ConfigManager —— 配置持久化管理
//!
//! 使用纯 `serde_json` 读写单个 `config.json`，并用单写者 `mpsc` channel 把写操作串行化，
//! 避免并发写互相覆盖。读取优先返回内存中的权威副本（与现网 Node 版 `config.json` 契约一致）。

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;

use crate::backend::constants::*;
use crate::backend::types::*;

/// 写操作指令（单写者 channel）
enum WriteOp {
    /// 将当前内存中的权威配置落盘
    Persist,
}

/// 配置持久化管理器
pub struct ConfigManager {
    config_dir: PathBuf,
    config_file: PathBuf,
    /// 内存中的权威配置副本
    data: Arc<Mutex<PersistedConfig>>,
    /// 串行化写操作的发送端
    writer_tx: mpsc::Sender<WriteOp>,
}

impl ConfigManager {
    pub fn new(config_dir: PathBuf) -> Self {
        let config_file = config_dir.join("config.json");
        let data = Arc::new(Mutex::new(PersistedConfig::default()));
        let (writer_tx, mut writer_rx) = mpsc::channel::<WriteOp>(32);

        // 单写者后台任务：串行消费写请求，把最新内存副本落盘
        let data_clone = data.clone();
        let file_clone = config_file.clone();
        tauri::async_runtime::spawn(async move {
            while writer_rx.recv().await.is_some() {
                let snapshot = { data_clone.lock().unwrap().clone() };
                if let Err(e) = persist(&file_clone, &snapshot) {
                    eprintln!("[ConfigManager] 持久化失败: {}", e);
                }
            }
        });

        Self {
            config_dir,
            config_file,
            data,
            writer_tx,
        }
    }

    /// 初始化：确保目录、迁移旧 config、读取并修复后写入默认文件
    pub fn init(&self) {
        let _ = std::fs::create_dir_all(&self.config_dir);

        // 首次运行：尝试从旧目录一次性迁移 config.json
        if !self.config_file.exists() {
            self.try_migrate();
        }

        let parsed = self.read_raw();
        let sanitized = parsed;
        {
            let mut g = self.data.lock().unwrap();
            *g = sanitized;
        }
        self.request_persist();
    }

    // ==================== 读取 ====================

    pub fn get_servers(&self) -> Vec<ServerConfig> {
        self.data.lock().unwrap().servers.clone()
    }

    pub fn get_server_by_id(&self, id: &str) -> Option<ServerConfig> {
        self.data
            .lock()
            .unwrap()
            .servers
            .iter()
            .find(|s| s.id == id)
            .cloned()
    }

    pub fn get_events(&self) -> Vec<EventConfig> {
        self.data.lock().unwrap().events.clone()
    }

    pub fn get_system_settings(&self) -> SystemSettings {
        self.data.lock().unwrap().system_settings.clone()
    }

    pub fn get_window_config(&self) -> WindowConfig {
        self.data.lock().unwrap().window_config.clone()
    }

    pub fn export_all(&self) -> PersistedConfig {
        self.data.lock().unwrap().clone()
    }

    // ==================== 写入 ====================

    pub fn save_servers(&self, servers: Vec<ServerConfig>) {
        self.update(|d| d.servers = servers);
    }

    pub fn save_events(&self, events: Vec<EventConfig>) {
        self.update(|d| d.events = events);
    }

    pub fn save_system_settings(&self, mut settings: SystemSettings) {
        // 数值防御性修正（缺失/越界回填默认）
        settings.heartbeat.ping_interval =
            clamp_or(settings.heartbeat.ping_interval, 30_000, PING_INTERVAL_MIN, PING_INTERVAL_MAX);
        settings.heartbeat.pong_timeout =
            clamp_or(settings.heartbeat.pong_timeout, 90_000, PONG_TIMEOUT_MIN, PONG_TIMEOUT_MAX);
        settings.log_retention_days =
            clamp_or(settings.log_retention_days, 7, LOG_RETENTION_MIN, LOG_RETENTION_MAX);
        settings.max_connections_per_server = clamp_or(
            settings.max_connections_per_server,
            1_000,
            MAX_CONNECTIONS_MIN,
            MAX_CONNECTIONS_MAX,
        );
        if settings.id.is_empty() {
            settings.id = "system".to_string();
        }
        settings.updated_at = now_rfc3339();
        self.update(|d| d.system_settings = settings);
    }

    pub fn save_window_config(&self, config: WindowConfig) {
        self.update(|d| d.window_config = config);
    }

    pub fn import_all(&self, config: PersistedConfig) {
        let _ = config.servers.len();
        self.update(|d| {
            d.servers = config.servers.clone();
            d.events = config.events.clone();
            d.templates = config.templates.clone();
            d.system_settings = config.system_settings.clone();
            d.window_config = config.window_config.clone();
            d.version = config.version.clone();
        });
    }

    // ==================== 内部方法 ====================

    /// 通用修改 + 触发落盘
    fn update<F: FnOnce(&mut PersistedConfig)>(&self, f: F) {
        {
            let mut g = self.data.lock().unwrap();
            f(&mut g);
            g.exported_at = now_rfc3339();
        }
        self.request_persist();
    }

    fn request_persist(&self) {
        // 非阻塞发送；缓冲区满时丢弃（下一次写会覆盖最新状态）
        let _ = self.writer_tx.try_send(WriteOp::Persist);
    }

    fn read_raw(&self) -> PersistedConfig {
        match std::fs::read_to_string(&self.config_file) {
            Ok(s) => match serde_json::from_str::<PersistedConfig>(&s) {
                Ok(mut p) => {
                    sanitize_in_place(&mut p);
                    p
                }
                Err(_) => PersistedConfig::default(),
            },
            Err(_) => PersistedConfig::default(),
        }
    }

    /// 旧目录 config.json 一次性迁移（失败忽略）
    fn try_migrate(&self) {
        let candidates: Vec<Option<PathBuf>> = vec![
            std::env::var("SSM_DATA_DIR")
                .ok()
                .map(|d| PathBuf::from(d).join("config").join("config.json")),
            std::env::current_dir()
                .ok()
                .map(|d| d.join("..").join("config").join("config.json")),
            Some(PathBuf::from("../config/config.json")),
            Some(PathBuf::from("../../config/config.json")),
        ];
        for cand in candidates.into_iter().flatten() {
            if cand.exists() {
                if std::fs::copy(&cand, &self.config_file).is_ok() {
                    println!(
                        "[ConfigManager] 已从旧目录迁移 config.json: {}",
                        cand.display()
                    );
                    return;
                }
            }
        }
    }
}

/// 数值 clamp：为 0（缺失）时回退默认，否则限制在 [min, max]
fn clamp_or(v: u64, default: u64, min: u64, max: u64) -> u64 {
    if v == 0 {
        default
    } else {
        v.max(min).min(max)
    }
}

/// 防御性修正：确保数组存在、数值合法
fn sanitize_in_place(p: &mut PersistedConfig) {
    if p.servers.is_empty() && p.version.is_empty() {
        // 允许空，无需处理
    }
    if p.system_settings.id.is_empty() {
        p.system_settings.id = "system".to_string();
    }
    p.system_settings.heartbeat.ping_interval = clamp_or(
        p.system_settings.heartbeat.ping_interval,
        30_000,
        PING_INTERVAL_MIN,
        PING_INTERVAL_MAX,
    );
    p.system_settings.heartbeat.pong_timeout = clamp_or(
        p.system_settings.heartbeat.pong_timeout,
        90_000,
        PONG_TIMEOUT_MIN,
        PONG_TIMEOUT_MAX,
    );
    p.system_settings.log_retention_days = clamp_or(
        p.system_settings.log_retention_days,
        7,
        LOG_RETENTION_MIN,
        LOG_RETENTION_MAX,
    );
    p.system_settings.max_connections_per_server = clamp_or(
        p.system_settings.max_connections_per_server,
        1_000,
        MAX_CONNECTIONS_MIN,
        MAX_CONNECTIONS_MAX,
    );
}

/// 将配置落盘（pretty JSON）
fn persist(file: &Path, data: &PersistedConfig) -> std::io::Result<()> {
    if let Some(parent) = file.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json = serde_json::to_string_pretty(data)?;
    std::fs::write(file, json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::ConfigManager;
    use crate::backend::types::*;

    use std::path::PathBuf;

    fn tmp_config_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ssm_cfg_{}", uuid::Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    fn mk_server(id: &str, name: &str) -> ServerConfig {
        ServerConfig {
            id: id.to_string(),
            name: name.to_string(),
            port: 3000,
            protocol: ProtocolType::Websocket,
            ..Default::default()
        }
    }

    #[test]
    fn save_and_get_servers_in_memory() {
        let dir = tmp_config_dir();
        let cm = ConfigManager::new(dir);
        cm.save_servers(vec![mk_server("a", "A"), mk_server("b", "B")]);
        let got = cm.get_servers();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].id, "a");
        assert_eq!(got[1].id, "b");
        let by_id = cm.get_server_by_id("a").expect("exists");
        assert_eq!(by_id.name, "A");
        assert!(cm.get_server_by_id("missing").is_none());
    }

    #[test]
    fn update_and_remove_server_roundtrip() {
        let dir = tmp_config_dir();
        let cm = ConfigManager::new(dir);
        cm.save_servers(vec![mk_server("a", "A")]);
        cm.save_servers(vec![mk_server("a", "A-updated")]);
        assert_eq!(cm.get_server_by_id("a").unwrap().name, "A-updated");
        cm.save_servers(vec![]);
        assert!(cm.get_server_by_id("a").is_none());
        assert!(cm.get_servers().is_empty());
    }

    #[test]
    fn servers_persist_to_temp_file() {
        let dir = tmp_config_dir();
        let servers = vec![mk_server("a", "A"), mk_server("b", "B")];
        {
            let cm = ConfigManager::new(dir.clone());
            cm.init();
            cm.save_servers(servers);
        }
        // async writer task flushes on drop; poll for the file to appear
        let file = dir.join("config.json");
        let mut written = false;
        for _ in 0..100 {
            if file.exists() {
                if let Ok(s) = std::fs::read_to_string(&file) {
                    if let Ok(pc) = serde_json::from_str::<PersistedConfig>(&s) {
                        if pc.servers.len() == 2 {
                            written = true;
                            break;
                        }
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(written, "config.json should persist 2 servers to temp file");
        let cm2 = ConfigManager::new(dir.clone());
        cm2.init();
        let reloaded = cm2.get_servers();
        assert_eq!(reloaded.len(), 2);
        assert_eq!(reloaded[0].id, "a");
    }

    #[test]
    fn export_import_roundtrip() {
        let dir = tmp_config_dir();
        let cm = ConfigManager::new(dir);
        let cfg = {
            let mut c = cm.export_all();
            c.servers = vec![mk_server("a", "A")];
            c
        };
        cm.import_all(cfg);
        let out = cm.export_all();
        assert_eq!(out.servers.len(), 1);
        assert_eq!(out.servers[0].id, "a");
    }
}

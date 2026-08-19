//! ConfigManager —— 配置持久化管理（v3 P0-1：存储后端可插拔）
//!
//! 持有 `Arc<dyn Repository>`，默认 `JsonRepository`（单 config.json，行为兼容旧版）。
//! 对外同步 API（`get_*` / `save_*` / `init` / `new`）保持不变，所有 REST/WS handler 零改动。
//! 存储切换（JSON ↔ SQLite）只在构造时决定；v3 P2 起默认启用 SQLite。
//!
//! 内存中维护 `PersistedConfig` 权威副本，单写者 `mpsc` channel 串行化落盘；
//! 合并写（dirty 标记）保证高频写不丢、无堆积。

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;

use crate::backend::constants::*;
use crate::backend::repository::{JsonRepository, Repository};
use crate::backend::types::*;

/// 写操作指令（单写者 channel）
enum WriteOp {
    /// 将当前内存中的权威配置落盘
    Persist,
}

/// 在「同步上下文」驱动一个 async future，且不依赖调用方是否已有 tokio runtime。
/// 做法：在独立 OS 线程上创建独立 multi_thread runtime 并 `block_on`，
/// 经 `std::sync::mpsc` 通道取回结果。彻底规避两类冲突：
/// - 调用方已有 runtime 时的「runtime within runtime」panic
/// - 单 worker multi_thread 下 `block_in_place` 的「not multi-threaded」报错
fn block_on_async<F, T>(fut: F) -> T
where
    F: std::future::Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("ConfigManager: 创建独立 runtime 失败");
        let result = rt.block_on(fut);
        let _ = tx.send(result);
    });
    rx.recv().expect("ConfigManager: 独立 runtime 线程异常")
}

/// 配置持久化管理器
pub struct ConfigManager {
    config_dir: PathBuf,
    config_file: PathBuf,
    /// 内存中的权威配置副本
    data: Arc<Mutex<PersistedConfig>>,
    /// 串行化写操作的发送端
    writer_tx: mpsc::Sender<WriteOp>,
    /// 可替换的存储后端
    repo: Arc<dyn Repository>,
    /// 合并写标记：存在未落盘的最新内存修改（背压兜底，替代静默丢弃）
    dirty: Arc<AtomicBool>,
    /// 飞行中的写信号：保证 channel 内最多 1 个 `Persist`，避免堆积
    in_flight: Arc<AtomicBool>,
}

impl ConfigManager {
    pub fn new(config_dir: PathBuf) -> Self {
        let config_file = config_dir.join("config.json");
        // P0：默认 JsonRepository（行为兼容旧版）。P2 起据 env/配置切换 SqliteRepository。
        let repo: Arc<dyn Repository> = Arc::new(JsonRepository::new(config_file.clone()));
        let data = Arc::new(Mutex::new(PersistedConfig::default()));
        let dirty = Arc::new(AtomicBool::new(false));
        let in_flight = Arc::new(AtomicBool::new(false));
        let (writer_tx, mut writer_rx) = mpsc::channel::<WriteOp>(4);

        // 单写者后台任务：串行落盘；合并写（dirty 标记）保证高频写不丢、无堆积
        let data_clone = data.clone();
        let repo_clone = repo.clone();
        let dirty_clone = dirty.clone();
        let in_flight_clone = in_flight.clone();
        tauri::async_runtime::spawn(async move {
            while writer_rx.recv().await.is_some() {
                // 合并写：只要等待期间又有新修改（dirty 被置位），就持续落盘最新内存状态，
                // 直到某次写盘后无新写入，把多次突发写合并为一次最终落盘。
                loop {
                    let snapshot = { data_clone.lock().unwrap().clone() };
                    if let Err(e) = repo_clone.save_config(&snapshot).await {
                        eprintln!("[ConfigManager] 持久化失败: {}", e);
                    }
                    if !dirty_clone.swap(false, Ordering::SeqCst) {
                        break;
                    }
                }
                in_flight_clone.store(false, Ordering::SeqCst);
            }
        });

        Self {
            config_dir,
            config_file,
            data,
            writer_tx,
            repo,
            dirty,
            in_flight,
        }
    }

    /// 初始化：确保目录、迁移旧 config、读取并修复后写入默认文件
    pub fn init(&self) {
        let _ = std::fs::create_dir_all(&self.config_dir);

        // 首次运行：尝试从旧目录一次性迁移 config.json（文件级兼容，复制到当前路径）
        if !self.config_file.exists() {
            self.try_migrate();
        }

        // 驱动异步 Repository：建表 → 从旧 JSON 迁移（若空）→ 载入内存。
        // future 持有 Arc<dyn Repository> 与 PathBuf 的副本，满足 'static + Send，
        // 可在独立 runtime 线程上安全驱动。
        let repo = self.repo.clone();
        let config_file = self.config_file.clone();
        let loaded = block_on_async(async move {
            repo.init().await?;
            repo.migrate_from_json(&config_file).await?;
            repo.load_config().await
        });
        match loaded {
            Ok(mut cfg) => {
                sanitize_in_place(&mut cfg);
                {
                    let mut g = self.data.lock().unwrap();
                    *g = cfg;
                }
            }
            Err(e) => {
                eprintln!("[ConfigManager] 载入配置失败，使用默认: {}", e);
            }
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

    // ===== Mock 服务 =====

    pub fn get_mock_services(&self) -> Vec<MockServiceConfig> {
        self.data.lock().unwrap().mock_services.clone()
    }

    pub fn get_mock_service_by_id(&self, id: &str) -> Option<MockServiceConfig> {
        self.data
            .lock()
            .unwrap()
            .mock_services
            .iter()
            .find(|s| s.id == id)
            .cloned()
    }

    pub fn save_mock_services(&self, list: Vec<MockServiceConfig>) {
        self.update(|d| d.mock_services = list);
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
            d.mock_services = config.mock_services.clone();
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
        // 标记需要写：合并写把多次高频写合并为一次最终落盘，绝不静默丢弃
        self.dirty.store(true, Ordering::SeqCst);
        // 仅当无飞行信号时投递一个 Persist，保证 channel 内最多 1 个信号、不堆积
        if self
            .in_flight
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            if self.writer_tx.try_send(WriteOp::Persist).is_err() {
                // 极端情况（写者已退出 / channel 关闭）：放开飞行标记，下次写会重试
                self.in_flight.store(false, Ordering::SeqCst);
            }
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

    #[test]
    fn legacy_config_with_removed_templates_field_migrates() {
        // P2-3（决策门 4 = B）：templates 已从 schema 移除。
        // 含 templates 的旧 config 应被 serde 忽略未知字段而成功加载（自动升级），而非报错。
        let json = r#"{
            "servers": [{"id":"a","name":"A","port":3000,"protocol":"websocket"}],
            "events": [],
            "templates": [{"id":"t1","name":"legacy"}],
            "mockServices": [],
            "systemSettings": {},
            "version": "1.0.0"
        }"#;
        let p: PersistedConfig = serde_json::from_str(json).expect("旧 config 应忽略 templates 自动升级");
        assert_eq!(p.servers.len(), 1);
        assert_eq!(p.servers[0].id, "a");
        assert_eq!(p.version, "1.0.0");
    }

    #[test]
    fn rapid_writes_coalesce_to_latest() {
        // F-5：高频连续写应合并为最新状态落盘，不应因通道满而静默丢弃
        let dir = tmp_config_dir();
        let cm = ConfigManager::new(dir.clone());
        cm.init();
        for i in 0..200 {
            cm.save_servers(vec![mk_server(&format!("s{}", i), &format!("N{}", i))]);
        }
        let file = dir.join("config.json");
        let mut ok = false;
        for _ in 0..300 {
            if file.exists() {
                if let Ok(s) = std::fs::read_to_string(&file) {
                    if let Ok(pc) = serde_json::from_str::<PersistedConfig>(&s) {
                        if let Some(last) = pc.servers.last() {
                            if last.id == "s199" {
                                ok = true;
                                break;
                            }
                        }
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(ok, "高频写应合并为最新状态落盘（s199）");
    }
}

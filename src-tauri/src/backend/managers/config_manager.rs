//! ConfigManager —— 配置持久化管理（v3 P0-1：存储后端可插拔；P2-2：去 JSON 直读）
//!
//! 持有 `Arc<dyn Repository>`，**主读 SQLite（`config.db`）**（`SqliteRepository`）。
//! 对外同步 API（`get_*` / `save_*` / `init` / `new`）保持不变，所有 REST/WS handler 零改动。
//!
//! ## 存储模式（env `SSM_REPOSITORY` 控制）
//! - `sqlite`（默认）：主读 SQLite（P2-2 目标态），首次空库自动从旧 `config.json` 迁移；
//!   SQLite 打开失败时强告警并紧急回退 `JsonRepository`（避免「数据看起来丢失」）。
//! - `json`：纯 JSON（逃生门，完全回退旧行为），仅用于紧急排障。
//!
//! `config.json` 仅作为「一次性迁移源」：空 SQLite 首次运行时导入，之后不再被主读/主写。
//!
//! 内存中维护 `PersistedConfig` 权威副本，单写者 `mpsc` channel 串行化落盘；
//! 合并写（dirty 标记）保证高频写不丢、无堆积。

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;

use crate::backend::constants::*;
use crate::backend::repository::{JsonRepository, Repository, SqliteRepository};
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
    /// 可替换的存储后端（主读写源）
    repo: Arc<dyn Repository>,
    /// 合并写标记：存在未落盘的最新内存修改（背压兜底，替代静默丢弃）
    dirty: Arc<AtomicBool>,
    /// 飞行中的写信号：保证 channel 内最多 1 个 `Persist`，避免堆积
    in_flight: Arc<AtomicBool>,
}

impl ConfigManager {
    /// 存储模式：env `SSM_REPOSITORY`（json | sqlite，默认 sqlite）
    fn repo_mode() -> String {
        std::env::var("SSM_REPOSITORY").unwrap_or_else(|_| "sqlite".to_string())
    }

    /// 构建主 repo（按模式；sqlite 用独立 runtime 驱动异步 open）
    fn build_repo(mode: &str, config_file: &PathBuf, config_dir: &PathBuf) -> Arc<dyn Repository> {
        match mode {
            "json" => Arc::new(JsonRepository::new(config_file.clone())),
            _ => {
                // 默认 sqlite：主读 SQLite（P2-2 目标态）；打开失败强告警 + JSON 兜底
                let db_path = config_dir.join("config.db");
                let repo = block_on_async(async move { SqliteRepository::open(&db_path).await });
                match repo {
                    Ok(r) => Arc::new(r),
                    Err(e) => {
                        eprintln!(
                            "[ConfigManager][紧急] config.db 打开失败，已回退 JSON 读取！\n  \
                             原因: {}\n  \
                             数据仍可从 config.json 读取，但 SQLite 主读不可用，请尽快检查磁盘/权限。",
                            e
                        );
                        Arc::new(JsonRepository::new(config_file.clone()))
                    }
                }
            }
        }
    }

    pub fn new(config_dir: PathBuf) -> Self {
        Self::with_mode(config_dir, &Self::repo_mode())
    }

    /// 指定存储模式构造（测试/程序化控制；`mode`: json | sqlite）
    pub fn with_mode(config_dir: PathBuf, mode: &str) -> Self {
        let config_file = config_dir.join("config.json");
        // P2-2：sqlite（默认）主读 SQLite；json 纯 JSON 回退（逃生门）
        let repo = Self::build_repo(mode, &config_file, &config_dir);
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

        // P2-2：不再有 dual 镜像。SQLite 已在上面 init+迁移+载入；内存权威副本已就绪。
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

    // ===== 场景编排（P1-3） =====

    pub fn get_scenes(&self) -> Vec<SceneConfig> {
        self.data.lock().unwrap().scenes.clone()
    }

    pub fn get_scene_by_id(&self, id: &str) -> Option<SceneConfig> {
        self.data
            .lock()
            .unwrap()
            .scenes
            .iter()
            .find(|s| s.id == id)
            .cloned()
    }

    pub fn save_scenes(&self, list: Vec<SceneConfig>) {
        self.update(|d| d.scenes = list);
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
            d.scenes = config.scenes.clone();
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
    use crate::backend::repository::{Repository, SqliteRepository};
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
            // 显式 json 模式，保留「JSON 落盘」语义（P2-2 默认已切 sqlite）
            let cm = ConfigManager::with_mode(dir.clone(), "json");
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
        assert!(written, "config.json should persist 2 servers to temp file (json mode)");
        let cm2 = ConfigManager::with_mode(dir.clone(), "json");
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
        let cm = ConfigManager::with_mode(dir.clone(), "json");
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
        assert!(ok, "高频写应合并为最新状态落盘（s199，json 模式）");
    }

    // ==================== P2-2 去 JSON 直读 ====================

    /// 默认模式（不设 env）应为 sqlite：数据落 config.db 且可主读
    #[tokio::test]
    async fn default_mode_is_sqlite() {
        let dir = tmp_config_dir();
        let cm = ConfigManager::new(dir.clone());
        cm.init();
        cm.save_servers(vec![mk_server("a", "A")]);
        let db = dir.join("config.db");
        let mut ok = false;
        for _ in 0..100 {
            std::thread::sleep(std::time::Duration::from_millis(30));
            if db.exists() {
                if let Ok(repo) = SqliteRepository::open(&db).await {
                    if let Ok(cfg) = repo.load_config().await {
                        if cfg.servers.iter().any(|s| s.id == "a") {
                            ok = true;
                            break;
                        }
                    }
                }
            }
        }
        assert!(ok, "默认 sqlite 模式应落库 config.db 且读到数据");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 旧配置全量迁移：json 模式写入（模拟旧版 config.json）→ sqlite 模式 init 读到全部数据
    #[tokio::test]
    async fn sqlite_mode_reads_migrated_data() {
        let dir = tmp_config_dir();
        // 用 json 模式写入（模拟旧版 config.json 落盘）
        {
            let cm = ConfigManager::with_mode(dir.clone(), "json");
            cm.init();
            cm.save_servers(vec![mk_server("s1", "S1")]);
            cm.save_system_settings(SystemSettings {
                id: "system".into(),
                start_minimized: true,
                ..Default::default()
            });
            let file = dir.join("config.json");
            for _ in 0..100 {
                if file.exists() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
        }
        // 切 sqlite 主读：init 从旧 config.json 迁移（空库导入）
        let cm = ConfigManager::with_mode(dir.clone(), "sqlite");
        cm.init();
        assert_eq!(cm.get_servers().len(), 1, "sqlite 模式应读到已迁移数据");
        assert_eq!(cm.get_server_by_id("s1").unwrap().name, "S1");
        assert!(
            cm.get_system_settings().start_minimized,
            "迁移应保留 systemSettings.startMinimized"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// JSON 比 SQLite 新（exported_at 比较）→ 全量覆盖导入，补全灰度期镜像落后
    #[tokio::test]
    async fn sqlite_migrate_overrides_stale_db() {
        let dir = tmp_config_dir();
        // 1) sqlite 模式先写「旧数据」
        {
            let cm = ConfigManager::with_mode(dir.clone(), "sqlite");
            cm.init();
            cm.save_servers(vec![mk_server("old", "Old")]);
            let db = dir.join("config.db");
            for _ in 0..100 {
                std::thread::sleep(std::time::Duration::from_millis(30));
                if db.exists() {
                    if let Ok(repo) = SqliteRepository::open(&db).await {
                        if let Ok(cfg) = repo.load_config().await {
                            if cfg.servers.iter().any(|s| s.id == "old") {
                                break;
                            }
                        }
                    }
                }
            }
        }
        // 2) 造一个 exported_at 更新的 config.json（模拟「更新的旧配置」）
        let json = r#"{
            "servers": [{"id":"new","name":"New","port":3000,"protocol":"websocket"}],
            "events": [],
            "mockServices": [],
            "scenes": [],
            "systemSettings": {"id":"system"},
            "windowConfig": {},
            "version": "2.0.0",
            "exportedAt": "2099-01-01T00:00:00Z"
        }"#;
        std::fs::write(dir.join("config.json"), json).unwrap();
        // 3) 再次 sqlite 模式 init：JSON 比 SQLite 新 → 全量覆盖
        let cm = ConfigManager::with_mode(dir.clone(), "sqlite");
        cm.init();
        let loaded = cm.get_servers();
        assert_eq!(loaded.len(), 1, "应被更新的 JSON 覆盖");
        assert_eq!(loaded[0].id, "new");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// SQLite 比 JSON 新（exported_at 比较）→ 不覆盖，验证「无数据丢失」反向
    #[tokio::test]
    async fn sqlite_keeps_newer_db() {
        let dir = tmp_config_dir();
        // 1) sqlite 模式写入数据（exported_at 为当前时间，较晚）
        {
            let cm = ConfigManager::with_mode(dir.clone(), "sqlite");
            cm.init();
            cm.save_servers(vec![mk_server("fresh", "Fresh")]);
            let db = dir.join("config.db");
            for _ in 0..100 {
                std::thread::sleep(std::time::Duration::from_millis(30));
                if db.exists() {
                    if let Ok(repo) = SqliteRepository::open(&db).await {
                        if let Ok(cfg) = repo.load_config().await {
                            if cfg.servers.iter().any(|s| s.id == "fresh") {
                                break;
                            }
                        }
                    }
                }
            }
        }
        // 2) 造一个 exported_at 更旧的 config.json（2020）
        let json = r#"{
            "servers": [{"id":"stale","name":"Stale","port":3000,"protocol":"websocket"}],
            "events": [],
            "mockServices": [],
            "scenes": [],
            "systemSettings": {"id":"system"},
            "windowConfig": {},
            "version": "2.0.0",
            "exportedAt": "2020-01-01T00:00:00Z"
        }"#;
        std::fs::write(dir.join("config.json"), json).unwrap();
        // 3) 再次 sqlite 模式 init：JSON 比 SQLite 旧 → 不覆盖
        let cm = ConfigManager::with_mode(dir.clone(), "sqlite");
        cm.init();
        let loaded = cm.get_servers();
        assert_eq!(loaded.len(), 1, "不应被旧 JSON 覆盖（无数据丢失）");
        assert_eq!(loaded[0].id, "fresh");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// json 模式：纯 JSON，不创建 SQLite（完全回退旧行为）
    #[test]
    fn json_mode_has_no_sqlite_mirror() {
        let dir = tmp_config_dir();
        let cm = ConfigManager::with_mode(dir.clone(), "json");
        cm.init();
        cm.save_servers(vec![mk_server("a", "A")]);
        let file = dir.join("config.json");
        for _ in 0..100 {
            if file.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
        assert!(file.exists(), "json 模式仍写 JSON");
        assert!(
            !dir.join("config.db").exists(),
            "json 模式不应创建 config.db"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

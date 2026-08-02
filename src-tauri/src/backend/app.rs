//! 后端聚合根（≡ Node `main.ts` 中的 App 实例）
//!
//! `Backend` 持有全部管理器与 EventBus，作为 axum 的共享状态（`AppState = Arc<Backend>`）。
//! `Backend::new` 在 Tauri `setup` 中构造（数据目录使用 `app_data_dir`，不再写相对路径）；
//! `run` 作为后台任务启动事件轮询、自动启动服务，并拉起 REST + 管理 WS 服务。

use std::sync::Arc;

use tauri::{AppHandle, Manager};

use crate::backend::api::router::build_router;
use crate::backend::constants::*;
use crate::backend::eventbus::EventBus;
use crate::backend::managers::client_manager::ClientManager;
use crate::backend::managers::config_manager::ConfigManager;
use crate::backend::managers::event_manager::EventManager;
use crate::backend::managers::log_manager::LogManager;
use crate::backend::managers::service_manager::ServiceManager;
use crate::backend::net::port_release::release_port;

/// 后端聚合根
#[derive(Clone)]
pub struct Backend {
    pub config: Arc<ConfigManager>,
    pub logs: Arc<LogManager>,
    pub clients: Arc<ClientManager>,
    pub events: Arc<EventManager>,
    pub services: Arc<ServiceManager>,
    pub event_bus: EventBus,
}

impl Backend {
    /// 在 Tauri setup 中构造后端（数据目录使用 app_data_dir）
    pub fn new(app: AppHandle) -> Self {
        let data_dir = app
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."));
        let config_dir = data_dir.join("config");
        let log_dir = data_dir.join("logs");

        let event_bus = EventBus::new();
        let config = Arc::new(ConfigManager::new(config_dir.clone()));
        config.init();
        let logs = Arc::new(LogManager::new(log_dir.clone(), event_bus.clone()));
        let clients = Arc::new(ClientManager::new());
        let services = Arc::new(ServiceManager::new(
            config.clone(),
            logs.clone(),
            clients.clone(),
            event_bus.clone(),
        ));
        services.reload();
        let events = Arc::new(EventManager::new(config.clone(), services.clone()));

        Self {
            config,
            logs,
            clients,
            events,
            services,
            event_bus,
        }
    }

    /// 测试专用构造：在不依赖 Tauri `AppHandle` 的前提下构建完整 `Backend`。
    ///
    /// 仅用于集成测试（自旋 axum router + 连接 `/admin/ws`）。生产路径仍走 `new(app)`。
    #[cfg(test)]
    pub fn new_test(data_dir: std::path::PathBuf) -> Self {
        let config_dir = data_dir.join("config");
        let log_dir = data_dir.join("logs");

        let event_bus = EventBus::new();
        let config = Arc::new(ConfigManager::new(config_dir.clone()));
        config.init();
        let logs = Arc::new(LogManager::new(log_dir.clone(), event_bus.clone()));
        let clients = Arc::new(ClientManager::new());
        let services = Arc::new(ServiceManager::new(
            config.clone(),
            logs.clone(),
            clients.clone(),
            event_bus.clone(),
        ));
        services.reload();
        let events = Arc::new(EventManager::new(config.clone(), services.clone()));

        Self {
            config,
            logs,
            clients,
            events,
            services,
            event_bus,
        }
    }

    /// 优雅关闭后端：停止全部受管服务。
    ///
    /// 事件轮询任务在独立 tokio 任务中运行且无外部中止句柄，进程退出时随进程结束；
    /// 此处显式停止全部 WsServer，确保端口被释放、连接被关闭。
    pub async fn shutdown(&self) {
        let _ = self.services.stop_all().await;
        println!("[backend] 已停止全部受管服务，准备退出");
    }
}

/// 启动后端后台任务：事件轮询 + 自动启动服务 + REST/WS 服务
pub async fn run(backend: Backend) {
    let state = Arc::new(backend);

    // 启动事件轮询
    state.events.start();

    // 自动启动标记为 auto_start 的服务
    for s in state.config.get_servers() {
        if s.auto_start {
            if let Err(e) = state.services.start(s.id.clone()).await {
                eprintln!("[backend] 自动启动服务 {} 失败: {}", s.id, e);
            }
        }
    }

    let app = build_router(state.clone());

    let port = api_port();
    let addr = (BIND_HOST, port);
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => Some(l),
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            eprintln!("[backend] REST 端口 {} 被占用，尝试释放后重试", port);
            release_port(port);
            tokio::time::sleep(std::time::Duration::from_millis(PORT_RELEASE_RETRY_DELAY_MS)).await;
            tokio::net::TcpListener::bind(addr).await.ok()
        }
        Err(e) => {
            eprintln!("[backend] 绑定 REST 端口失败: {}", e);
            None
        }
    };

    match listener {
        Some(listener) => {
            println!("[backend] REST API + 管理 WS 监听 http://{}:{}", BIND_HOST, port);
            if let Err(e) = axum::serve(listener, app).await {
                eprintln!("[backend] REST 服务异常退出: {}", e);
            }
        }
        None => {
            eprintln!("[backend] 无法启动 REST API 服务");
        }
    }
}

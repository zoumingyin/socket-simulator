//! 后端聚合根（≡ Node `main.ts` 中的 App 实例）
//!
//! `Backend` 持有全部管理器与 EventBus，作为 axum 的共享状态（`AppState = Arc<Backend>`）。
//! `Backend::new` 在 Tauri `setup` 中构造（数据目录使用 `app_data_dir`，不再写相对路径）；
//! `run` 作为后台任务启动事件轮询、自动启动服务，并拉起 REST + 管理 WS 服务。

use std::sync::Arc;

use tauri::{AppHandle, Manager};

use tower_http::cors::CorsLayer;

use crate::backend::api::router::build_router;
use crate::backend::auth::AuthManager;
use crate::backend::constants::*;
use crate::backend::eventbus::EventBus;
use crate::backend::managers::client_manager::ClientManager;
use crate::backend::managers::config_manager::ConfigManager;
use crate::backend::managers::event_manager::EventManager;
use crate::backend::managers::log_manager::LogManager;
use crate::backend::managers::service_manager::ServiceManager;
use crate::backend::mock::MockManager;
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
    pub mock: Arc<MockManager>,
    pub auth: Arc<AuthManager>,
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
        let mock = Arc::new(MockManager::new());
        let auth = AuthManager::new_blocking(&data_dir);

        Self {
            config,
            logs,
            clients,
            events,
            services,
            event_bus,
            mock,
            auth,
        }
    }

    /// 测试专用构造
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
        let mock = Arc::new(MockManager::new());
        let auth = AuthManager::new_blocking(&data_dir);

        Self {
            config,
            logs,
            clients,
            events,
            services,
            event_bus,
            mock,
            auth,
        }
    }

    /// 优雅关闭后端
    pub async fn shutdown(&self) {
        let _ = self.services.stop_all().await;
        self.mock.stop_all().await;
        println!("[backend] 已停止全部服务，准备退出");
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

    // 恢复 Mock 服务（主端口的标记；自定义端口的立即启动 listener）
    let sys = state.config.get_system_settings();

    // F-3: 启动后按保留天数清理过期日志文件
    let removed = state.logs.cleanup_old(sys.log_retention_days);
    if removed > 0 {
        println!(
            "[backend] 已清理 {} 个过期日志文件（保留 {} 天）",
            removed, sys.log_retention_days
        );
    }

    state.mock.restore(&state.config, &sys).await;

    // 构建主路由：显式 API + WS + 统一 fallback（Mock → 前端静态文件 → SPA index.html）
    //
    // 路由优先级：
    //   1. /api/*         → REST API（显式路由）
    //   2. /admin/ws      → 管理 WebSocket（显式路由）
    //   3. Mock basePath  → Mock 引擎分发（dispatch_main_port 返回 Some 时）
    //   4. 静态文件        → dist/assets/*.js|css|...（frontend::serve）
    //   5. SPA fallback   → index.html（React Router 客户端路由）
    let app = build_router(state.clone())
        .fallback(move |req: axum::extract::Request| {
            let cfg = state.config.clone();
            let sys = state.config.get_system_settings();
            let mock = state.mock.clone();
            async move {
                // 提取 path（dispatch_main_port 会消费 req）
                let path = req.uri().path().to_string();

                // 1. 尝试 Mock 分发（仅主端口 Mock 服务）
                if let Some(resp) = mock.dispatch_main_port(&cfg, &sys, req).await {
                    return resp;
                }

                // 2. 前端静态文件 / SPA 回退
                crate::backend::frontend::serve(&path)
            }
        })
        .layer(CorsLayer::permissive());

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
            let frontend_status = if crate::backend::frontend::is_embedded() {
                "已嵌入"
            } else {
                "未构建（dist/ 为空）"
            };
            println!(
                "[backend] REST API + 管理 WS + Mock + 前端({}) 监听 http://{}:{}",
                frontend_status, BIND_HOST, port
            );
            if let Err(e) = axum::serve(listener, app).await {
                eprintln!("[backend] REST 服务异常退出: {}", e);
            }
        }
        None => {
            eprintln!("[backend] 无法启动 REST API 服务");
        }
    }
}

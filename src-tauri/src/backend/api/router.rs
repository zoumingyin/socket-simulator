//! REST API 路由表（端口 3080）
//!
//! 覆盖 Node `backend/api/index.ts` 的全部路由，并挂载管理端 WS 通道 `/admin/ws`。
//! 状态类型 `AppState = Arc<Backend>`，由 `app::run` 通过 `with_state` 注入。

use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::CorsLayer;

use crate::backend::api::handlers;
use crate::backend::constants::ADMIN_WS_PATH;
use crate::backend::state::AppState;
use crate::backend::ws::admin;

/// 构建路由（状态由调用方通过 `with_state` 注入）
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/api/servers", get(handlers::get_servers))
        .route("/api/server/list", get(handlers::get_server_list))
        .route("/api/server/runtimes", get(handlers::get_server_runtimes))
        .route("/api/server/add", post(handlers::server_add))
        .route("/api/server/update", post(handlers::server_update))
        .route("/api/server/remove", post(handlers::server_remove))
        .route("/api/server/start", post(handlers::server_start))
        .route("/api/server/stop", post(handlers::server_stop))
        .route("/api/server/restart", post(handlers::server_restart))
        .route("/api/server/start-all", post(handlers::server_start_all))
        .route("/api/server/stop-all", post(handlers::server_stop_all))
        .route("/api/server/restart-all", post(handlers::server_restart_all))
        .route("/api/events", get(handlers::get_events))
        .route("/api/events/add", post(handlers::event_add))
        .route("/api/events/update", post(handlers::event_update))
        .route("/api/events/remove", post(handlers::event_remove))
        .route("/api/events/toggle", post(handlers::event_toggle))
        .route("/api/clients", get(handlers::get_clients))
        .route("/api/client/disconnect", post(handlers::client_disconnect))
        .route("/api/client/send", post(handlers::client_send))
        .route("/api/send-message", post(handlers::send_message))
        .route("/api/logs", get(handlers::get_logs))
        .route("/api/logs/clear", post(handlers::logs_clear))
        .route("/api/settings", get(handlers::get_settings))
        .route("/api/settings", post(handlers::save_settings))
        .route("/api/export", get(handlers::export_config))
        .route("/api/import", post(handlers::import_config))
        .route(ADMIN_WS_PATH, get(admin::admin_ws))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

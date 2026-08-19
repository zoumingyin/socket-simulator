//! REST API 路由表（端口 3080）
//!
//! 覆盖 Node `backend/api/index.ts` 的全部路由，并挂载管理端 WS 通道 `/admin/ws`。
//! 状态类型 `AppState = Arc<Backend>`，由 `app::run` 通过 `with_state` 注入。

use axum::middleware::from_fn_with_state;
use axum::routing::{get, post};
use axum::Router;

use crate::backend::api::handlers;
use crate::backend::auth::{auth_middleware, bootstrap};
use crate::backend::constants::ADMIN_WS_PATH;
use crate::backend::openapi::openapi_json;
use crate::backend::state::AppState;
use crate::backend::ws::admin;

/// 构建路由（状态由调用方通过 `with_state` 注入）
pub fn build_router(state: AppState) -> Router {
    // 鉴权自举端点：独立 state（Arc<AuthManager>），与全局 AppState 解耦
    let auth_routes = Router::new()
        .route("/api/auth/bootstrap", get(bootstrap))
        .with_state(state.auth.clone());

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
        // 审计日志
        .route("/api/audit/logs", get(handlers::audit_logs))
        // OpenAPI 3.1 文档（P0-4；swagger-ui 可另行挂载）
        .route("/api/openapi.json", get(|| async { openapi_json() }))
        // Mock 服务
        .route("/api/mock/list", get(handlers::mock_list))
        .route("/api/mock/get", post(handlers::mock_get))
        .route("/api/mock/add", post(handlers::mock_add))
        .route("/api/mock/update", post(handlers::mock_update))
        .route("/api/mock/remove", post(handlers::mock_remove))
        .route("/api/mock/start", post(handlers::mock_start))
        .route("/api/mock/stop", post(handlers::mock_stop))
        // 场景编排（P1-3）
        .route("/api/scene/list", get(handlers::scene_list))
        .route("/api/scene/add", post(handlers::scene_add))
        .route("/api/scene/update", post(handlers::scene_update))
        .route("/api/scene/remove", post(handlers::scene_remove))
        .route("/api/scene/start", post(handlers::scene_start))
        .route("/api/scene/stop", post(handlers::scene_stop))
        .route(ADMIN_WS_PATH, get(admin::admin_ws))
        .merge(auth_routes)
        .with_state(state.clone())
        // 鉴权中间件：默认关闭（SSM_AUTH_ENABLED=1 启用）；enabled 时校验 Bearer token
        .layer(from_fn_with_state(state.auth.clone(), auth_middleware))
}

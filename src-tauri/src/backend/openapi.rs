//! v3 P0-4 OpenAPI 3.1 文档聚合
//!
//! 通过 `#[utoipa::path]` 注解从 handler 生成规范（utoipa 5.x 默认输出 **OpenAPI 3.1**）。
//! 访问：
//! - 运行时：`GET /api/openapi.json`
//! - 离线导出：`cargo run --bin export_openapi` → `openapi.json`（供 openapi-typescript 生成前端类型）
//!
//! 契约权威源（决策门 2，2026-08-19）：OpenAPI 3.1 为权威；F-4 specta `generated.ts`
//! 在 v3 迁移期保留作过渡，本管道完成后退役。

use utoipa::OpenApi;

use crate::backend::api::handlers::*;
use crate::backend::audit::{AuditEntry, AuditPage, AuditQuery};
use crate::backend::types::*;

/// OpenAPI 文档根（路径经 `#[utoipa::path]` 注解聚合）
#[derive(OpenApi)]
#[openapi(
    info(
        title = "NexSocket Studio · 管理面 API",
        description = "Socket 服务管理平台后端 REST API（OpenAPI 3.1）。\
            鉴权：Bearer token（`SSM_AUTH_ENABLED=1` 时生效）；\
            `GET /api/auth/bootstrap` 回环自举获取 admin token。",
        version = "3.0.0",
    ),
    servers(
        (url = "http://127.0.0.1:3080", description = "本地回环（默认绑定）")
    ),
    paths(
        get_servers,
        get_server_list,
        get_server_runtimes,
        server_add,
        server_update,
        server_remove,
        server_start,
        server_stop,
        server_restart,
        server_start_all,
        server_stop_all,
        server_restart_all,
        get_events,
        event_add,
        event_update,
        event_remove,
        event_toggle,
        get_clients,
        client_disconnect,
        client_send,
        send_message,
        get_logs,
        logs_clear,
        get_settings,
        save_settings,
        export_config,
        import_config,
        mock_list,
        mock_get,
        mock_add,
        mock_update,
        mock_remove,
        mock_start,
        mock_stop,
        audit_logs,
        crate::backend::auth::bootstrap,
    ),
    components(schemas(
        ApiResponse::<serde_json::Value>,
        PersistedConfig,
        ServerConfig,
        ServerRuntime,
        ServerStatus,
        ProtocolType,
        LogLevel,
        EventConfig,
        EventStatus,
        MockServiceConfig,
        MockRule,
        MockMatchCondition,
        HttpRouteConfig,
        HttpMethod,
        HttpRouteType,
        SystemSettings,
        WindowConfig,
        HeartbeatConfig,
        WssConfig,
        IpAccessList,
        ClientInfo,
        ClientStatus,
        ClientGroupType,
        LogEntry,
        AuditEntry,
        AuditPage,
        AuditQuery,
        crate::backend::api::handlers::ServerId,
        crate::backend::api::handlers::EventId,
        crate::backend::api::handlers::EventToggle,
        crate::backend::api::handlers::ClientDisconnect,
        crate::backend::api::handlers::SendBody,
        crate::backend::api::handlers::SettingsBody,
        crate::backend::api::handlers::MockId,
    ))
)]
pub struct ApiDoc;

/// 序列化 spec 为 JSON（`/api/openapi.json` 端点与 `export_openapi` bin 共用）
pub fn openapi_json() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::to_value(ApiDoc::openapi()).unwrap_or(serde_json::Value::Null))
}

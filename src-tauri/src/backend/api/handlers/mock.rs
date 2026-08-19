//! Mock 服务处理函数（/api/mock/*）
//!
//! 业务编排（校验 / 持久化 / 启停）下沉到 `MockManager` 门面，
//! 本模块仅负责请求解析与响应映射，不依赖完整 axum 内部状态。

use axum::extract::{Json, State};
use axum::http::HeaderMap;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::backend::api::handlers::{audit_log, err, ok, ok_msg, ok_msg_only, Resp};
use crate::backend::state::AppState;
use crate::backend::types::*;

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MockId {
    id: String,
}

/// Mock 服务配置摘要（审计 detail 用）
fn mock_summary(c: &MockServiceConfig) -> Value {
    json!({
        "name": c.name,
        "basePath": c.base_path,
        "customPort": c.custom_port,
        "enabled": c.enabled,
    })
}

#[utoipa::path(
    get,
    path = "/api/mock/list",
    responses((status = 200, description = "OK"))
)]
pub async fn mock_list(State(b): State<AppState>) -> Resp {
    let services = b.config.get_mock_services();
    ok(serde_json::to_value(services).unwrap_or(Value::Null))
}

#[utoipa::path(
    post,
    path = "/api/mock/get",
    request_body = MockId,
    responses((status = 200, description = "OK"))
)]
pub async fn mock_get(State(b): State<AppState>, Json(body): Json<MockId>) -> Resp {
    match b.config.get_mock_service_by_id(&body.id) {
        Some(s) => ok(serde_json::to_value(s).unwrap_or(Value::Null)),
        None => err("MOCK_NOT_FOUND", format!("未找到 mock 服务 {}", body.id), 404),
    }
}

#[utoipa::path(
    post,
    path = "/api/mock/add",
    request_body = MockServiceConfig,
    responses((status = 200, description = "OK"))
)]
pub async fn mock_add(
    State(b): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<MockServiceConfig>,
) -> Resp {
    let sys = b.config.get_system_settings();
    match b.mock.add_service(body, &b.config, &sys).await {
        Ok(cfg) => {
            audit_log(
                &b,
                &headers,
                "mock_add",
                "mock",
                Some(cfg.id.clone()),
                mock_summary(&cfg),
                true,
            )
            .await;
            ok_msg(serde_json::to_value(cfg).unwrap_or(Value::Null), "添加成功")
        }
        Err(e) => {
            audit_log(
                &b,
                &headers,
                "mock_add",
                "mock",
                None,
                json!({ "error": e.message }),
                false,
            )
            .await;
            err(e.code, e.message, e.status)
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/mock/update",
    request_body = MockServiceConfig,
    responses((status = 200, description = "OK"))
)]
pub async fn mock_update(
    State(b): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<MockServiceConfig>,
) -> Resp {
    let sys = b.config.get_system_settings();
    match b.mock.update_service(body, &b.config, &sys).await {
        Ok(cfg) => {
            audit_log(
                &b,
                &headers,
                "mock_update",
                "mock",
                Some(cfg.id.clone()),
                mock_summary(&cfg),
                true,
            )
            .await;
            ok_msg(serde_json::to_value(cfg).unwrap_or(Value::Null), "更新成功")
        }
        Err(e) => {
            audit_log(
                &b,
                &headers,
                "mock_update",
                "mock",
                None,
                json!({ "error": e.message }),
                false,
            )
            .await;
            err(e.code, e.message, e.status)
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/mock/remove",
    request_body = MockId,
    responses((status = 200, description = "OK"))
)]
pub async fn mock_remove(
    State(b): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<MockId>,
) -> Resp {
    b.mock.remove_service(&body.id, &b.config).await;
    audit_log(
        &b,
        &headers,
        "mock_remove",
        "mock",
        Some(body.id.clone()),
        json!({}),
        true,
    )
    .await;
    ok_msg_only("删除成功")
}

#[utoipa::path(
    post,
    path = "/api/mock/start",
    request_body = MockId,
    responses((status = 200, description = "OK"))
)]
pub async fn mock_start(
    State(b): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<MockId>,
) -> Resp {
    let sys = b.config.get_system_settings();
    match b.mock.start_service(&body.id, &b.config, &sys).await {
        Ok(port) => {
            audit_log(
                &b,
                &headers,
                "mock_start",
                "mock",
                Some(body.id.clone()),
                json!({ "port": port }),
                true,
            )
            .await;
            ok(serde_json::json!({ "port": port }))
        }
        Err(e) => {
            audit_log(
                &b,
                &headers,
                "mock_start",
                "mock",
                Some(body.id.clone()),
                json!({ "error": e.message }),
                false,
            )
            .await;
            err(e.code, e.message, e.status)
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/mock/stop",
    request_body = MockId,
    responses((status = 200, description = "OK"))
)]
pub async fn mock_stop(
    State(b): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<MockId>,
) -> Resp {
    b.mock.stop_service(&body.id).await;
    audit_log(
        &b,
        &headers,
        "mock_stop",
        "mock",
        Some(body.id.clone()),
        json!({}),
        true,
    )
    .await;
    ok_msg_only("停止成功")
}

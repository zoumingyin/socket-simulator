//! Mock 服务处理函数（/api/mock/*）
//!
//! 业务编排（校验 / 持久化 / 启停）下沉到 `MockManager` 门面，
//! 本模块仅负责请求解析与响应映射，不依赖完整 axum 内部状态。

use axum::extract::{Json, State};
use serde::Deserialize;
use serde_json::Value;

use crate::backend::api::handlers::{err, ok, ok_msg, ok_msg_only, Resp};
use crate::backend::state::AppState;
use crate::backend::types::*;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MockId {
    id: String,
}

pub async fn mock_list(State(b): State<AppState>) -> Resp {
    let services = b.config.get_mock_services();
    ok(serde_json::to_value(services).unwrap_or(Value::Null))
}

pub async fn mock_get(State(b): State<AppState>, Json(body): Json<MockId>) -> Resp {
    match b.config.get_mock_service_by_id(&body.id) {
        Some(s) => ok(serde_json::to_value(s).unwrap_or(Value::Null)),
        None => err("MOCK_NOT_FOUND", format!("未找到 mock 服务 {}", body.id), 404),
    }
}

pub async fn mock_add(State(b): State<AppState>, Json(body): Json<MockServiceConfig>) -> Resp {
    let sys = b.config.get_system_settings();
    match b.mock.add_service(body, &b.config, &sys).await {
        Ok(cfg) => ok_msg(serde_json::to_value(cfg).unwrap_or(Value::Null), "添加成功"),
        Err(e) => err(e.code, e.message, e.status),
    }
}

pub async fn mock_update(State(b): State<AppState>, Json(body): Json<MockServiceConfig>) -> Resp {
    let sys = b.config.get_system_settings();
    match b.mock.update_service(body, &b.config, &sys).await {
        Ok(cfg) => ok_msg(serde_json::to_value(cfg).unwrap_or(Value::Null), "更新成功"),
        Err(e) => err(e.code, e.message, e.status),
    }
}

pub async fn mock_remove(State(b): State<AppState>, Json(body): Json<MockId>) -> Resp {
    b.mock.remove_service(&body.id, &b.config).await;
    ok_msg_only("删除成功")
}

pub async fn mock_start(State(b): State<AppState>, Json(body): Json<MockId>) -> Resp {
    let sys = b.config.get_system_settings();
    match b.mock.start_service(&body.id, &b.config, &sys).await {
        Ok(port) => ok(serde_json::json!({ "port": port })),
        Err(e) => err(e.code, e.message, e.status),
    }
}

pub async fn mock_stop(State(b): State<AppState>, Json(body): Json<MockId>) -> Resp {
    b.mock.stop_service(&body.id).await;
    ok_msg_only("停止成功")
}

//! REST API 处理函数（≡ Node `backend/api/index.ts`）
//!
//! 每个处理函数对应 Node 的一个路由分支。统一返回 `(StatusCode, Json<ApiResponse<Value>>)`，
//! 与现网 `{ success, data?, errorCode?, error?, message?, timestamp }` 契约一致。

use std::collections::HashMap;

use axum::extract::{Json, Query, State};
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::Value;

use crate::backend::constants::*;
use crate::backend::state::AppState;
use crate::backend::types::*;

type Resp = (StatusCode, axum::Json<ApiResponse<Value>>);

// ==================== 响应辅助 ====================

fn ok(data: Value) -> Resp {
    (
        StatusCode::OK,
        axum::Json(ApiResponse::success(data, None)),
    )
}

fn ok_msg(data: Value, msg: &str) -> Resp {
    (
        StatusCode::OK,
        axum::Json(ApiResponse::success(data, Some(msg.to_string()))),
    )
}

fn ok_msg_only(msg: &str) -> Resp {
    let r = ApiResponse {
        success: true,
        data: None,
        error_code: None,
        error: None,
        message: Some(msg.to_string()),
        timestamp: now_rfc3339(),
    };
    (StatusCode::OK, axum::Json(r))
}

fn err(code: &str, msg: String, status: u16) -> Resp {
    let r = ApiResponse {
        success: false,
        data: None,
        error_code: Some(code.to_string()),
        error: Some(msg),
        message: None,
        timestamp: now_rfc3339(),
    };
    (
        StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        axum::Json(r),
    )
}

// ==================== 请求体 ====================

#[derive(Deserialize)]
pub(crate) struct ServerId {
    id: String,
}

#[derive(Deserialize)]
pub(crate) struct EventId {
    id: String,
}

#[derive(Deserialize)]
pub(crate) struct EventToggle {
    id: String,
    status: String,
}

#[derive(Deserialize)]
pub(crate) struct ClientDisconnect {
    client_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SendBody {
    server_id: Option<String>,
    target_type: Option<String>,
    target_id: Option<String>,
    event: String,
    message_type: Option<String>,
    content: Option<String>,
    data: Option<Value>,
    client_id: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct SettingsBody {
    system_settings: Option<SystemSettings>,
    window_config: Option<WindowConfig>,
}

// ==================== 服务管理 ====================

pub async fn get_servers(State(b): State<AppState>) -> Resp {
    let configs = b.config.get_servers();
    let runtimes = b.services.get_runtimes();
    ok(serde_json::json!({ "configs": configs, "runtimes": runtimes }))
}

pub async fn get_server_list(State(b): State<AppState>) -> Resp {
    ok(serde_json::to_value(b.config.get_servers()).unwrap_or(Value::Null))
}

pub async fn get_server_runtimes(State(b): State<AppState>) -> Resp {
    ok(serde_json::to_value(b.services.get_runtimes()).unwrap_or(Value::Null))
}

pub async fn server_add(State(b): State<AppState>, Json(mut body): Json<ServerConfig>) -> Resp {
    if body.id.is_empty() {
        body.id = uuid::Uuid::new_v4().to_string();
    }
    if body.created_at.is_empty() {
        body.created_at = now_rfc3339();
    }
    if body.updated_at.is_empty() {
        body.updated_at = now_rfc3339();
    }
    b.services.register_server(body.clone());
    let mut servers = b.config.get_servers();
    servers.push(body.clone());
    b.config.save_servers(servers);
    ok_msg(
        serde_json::to_value(body).unwrap_or(Value::Null),
        "添加成功",
    )
}

pub async fn server_update(State(b): State<AppState>, Json(body): Json<ServerConfig>) -> Resp {
    b.services.register_server(body.clone());
    let mut servers = b.config.get_servers();
    match servers.iter().position(|s| s.id == body.id) {
        Some(i) => servers[i] = body,
        None => servers.push(body),
    }
    b.config.save_servers(servers);
    ok_msg_only("更新成功")
}

pub async fn server_remove(State(b): State<AppState>, Json(body): Json<ServerId>) -> Resp {
    if !b.services.remove_server(&body.id) {
        return err("SERVER_RUNNING", "服务正在运行，无法删除".into(), 400);
    }
    let servers = b
        .config
        .get_servers()
        .into_iter()
        .filter(|s| s.id != body.id)
        .collect();
    b.config.save_servers(servers);
    ok_msg_only("删除成功")
}

pub async fn server_start(State(b): State<AppState>, Json(body): Json<ServerId>) -> Resp {
    match b.services.start(body.id.clone()).await {
        Ok(()) => ok_msg_only("启动成功"),
        Err(e) => err(e.error_code(), e.to_string(), e.status_code()),
    }
}

pub async fn server_stop(State(b): State<AppState>, Json(body): Json<ServerId>) -> Resp {
    match b.services.stop(&body.id).await {
        Ok(()) => ok_msg_only("停止成功"),
        Err(e) => err(e.error_code(), e.to_string(), e.status_code()),
    }
}

pub async fn server_restart(State(b): State<AppState>, Json(body): Json<ServerId>) -> Resp {
    match b.services.restart(body.id.clone()).await {
        Ok(()) => ok_msg_only("重启成功"),
        Err(e) => err(e.error_code(), e.to_string(), e.status_code()),
    }
}

pub async fn server_start_all(State(b): State<AppState>) -> Resp {
    match b.services.start_all().await {
        Ok(()) => ok_msg_only("全部启动成功"),
        Err(e) => err(e.error_code(), e.to_string(), e.status_code()),
    }
}

pub async fn server_stop_all(State(b): State<AppState>) -> Resp {
    match b.services.stop_all().await {
        Ok(()) => ok_msg_only("全部停止成功"),
        Err(e) => err(e.error_code(), e.to_string(), e.status_code()),
    }
}

pub async fn server_restart_all(State(b): State<AppState>) -> Resp {
    match b.services.restart_all().await {
        Ok(()) => ok_msg_only("全部重启成功"),
        Err(e) => err(e.error_code(), e.to_string(), e.status_code()),
    }
}

// ==================== 事件管理 ====================

pub async fn get_events(
    State(b): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> Resp {
    let mut events = b.config.get_events();
    if let Some(sid) = q.get("serverId") {
        events.retain(|e| &e.server_id == sid);
    }
    ok(serde_json::to_value(events).unwrap_or(Value::Null))
}

pub async fn event_add(State(b): State<AppState>, Json(body): Json<EventConfig>) -> Resp {
    let evt = b.events.add_event(body);
    ok_msg(
        serde_json::to_value(evt).unwrap_or(Value::Null),
        "事件添加成功",
    )
}

pub async fn event_update(State(b): State<AppState>, Json(body): Json<EventConfig>) -> Resp {
    match b.events.update_event(&body.id, body.clone()) {
        Some(evt) => ok_msg(
            serde_json::to_value(evt).unwrap_or(Value::Null),
            "事件更新成功",
        ),
        None => err("EVENT_NOT_FOUND", "事件不存在".into(), 404),
    }
}

pub async fn event_remove(State(b): State<AppState>, Json(body): Json<EventId>) -> Resp {
    if b.events.remove_event(&body.id) {
        ok_msg_only("事件删除成功")
    } else {
        err("EVENT_NOT_FOUND", "事件不存在".into(), 404)
    }
}

pub async fn event_toggle(State(b): State<AppState>, Json(body): Json<EventToggle>) -> Resp {
    let status = if body.status == "enabled" {
        EventStatus::Enabled
    } else {
        EventStatus::Disabled
    };
    match b.events.toggle_event(&body.id, status) {
        Some(evt) => ok_msg(
            serde_json::to_value(evt).unwrap_or(Value::Null),
            "状态切换成功",
        ),
        None => err("EVENT_NOT_FOUND", "事件不存在".into(), 404),
    }
}

// ==================== 客户端管理 ====================

pub async fn get_clients(
    State(b): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> Resp {
    let server_id = q.get("serverId").map(|s| s.as_str());
    let clients = b.clients.get_clients(server_id);
    ok(serde_json::to_value(clients).unwrap_or(Value::Null))
}

pub async fn client_disconnect(
    State(b): State<AppState>,
    Json(body): Json<ClientDisconnect>,
) -> Resp {
    let (server_id, actual) = match body.client_id.split_once(CLIENT_ID_SEP) {
        Some((s, c)) => (s.to_string(), c.to_string()),
        None => (String::new(), body.client_id.clone()),
    };
    match b.services.disconnect_client(&server_id, &actual).await {
        Ok(()) => ok_msg_only("客户端已断开"),
        Err(e) => err(e.error_code(), e.to_string(), e.status_code()),
    }
}

pub async fn client_send(State(b): State<AppState>, Json(body): Json<SendBody>) -> Resp {
    send_via(b, body).await
}

pub async fn send_message(State(b): State<AppState>, Json(body): Json<SendBody>) -> Resp {
    send_via(b, body).await
}

async fn send_via(b: AppState, body: SendBody) -> Resp {
    let server_id = body
        .server_id
        .clone()
        .or_else(|| {
            body.client_id
                .clone()
                .and_then(|c| c.split_once(CLIENT_ID_SEP).map(|(s, _)| s.to_string()))
        })
        .unwrap_or_default();
    if server_id.is_empty() {
        return err("SERVER_NOT_FOUND", "无法确定服务 ID".into(), 400);
    }
    let data: Value = if let Some(d) = body.data {
        d
    } else if let Some(content) = body.content {
        if body.message_type.as_deref() == Some("json") {
            serde_json::from_str(&content).unwrap_or(Value::String(content))
        } else {
            Value::String(content)
        }
    } else {
        Value::Null
    };
    let target_type = body.target_type.unwrap_or_else(|| "broadcast".to_string());
    match b
        .services
        .send_message(&server_id, &target_type, body.target_id.as_deref(), &body.event, data)
        .await
    {
        Ok(()) => ok_msg_only("消息已发送"),
        Err(e) => err(e.error_code(), e.to_string(), e.status_code()),
    }
}

// ==================== 日志 ====================

pub async fn get_logs(
    State(b): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> Resp {
    let level = q
        .get("level")
        .and_then(|l| serde_json::from_value::<LogLevel>(Value::String(l.clone())).ok());
    let filter = LogFilter {
        server_id: q.get("serverId").cloned(),
        level,
        keyword: q.get("keyword").cloned(),
        ..Default::default()
    };
    let entries = b.logs.get_entries(&filter);
    ok(serde_json::to_value(entries).unwrap_or(Value::Null))
}

pub async fn logs_clear(State(b): State<AppState>) -> Resp {
    b.logs.clear_entries();
    ok_msg_only("日志已清空")
}

// ==================== 系统设置 ====================

pub async fn get_settings(State(b): State<AppState>) -> Resp {
    let settings = b.config.get_system_settings();
    let window = b.config.get_window_config();
    ok(serde_json::json!({ "systemSettings": settings, "windowConfig": window }))
}

pub async fn save_settings(State(b): State<AppState>, Json(body): Json<SettingsBody>) -> Resp {
    if let Some(s) = body.system_settings {
        b.config.save_system_settings(s);
    }
    if let Some(w) = body.window_config {
        b.config.save_window_config(w);
    }
    ok_msg_only("设置已保存")
}

// ==================== 配置导入/导出 ====================

pub async fn export_config(State(b): State<AppState>) -> Resp {
    ok(serde_json::to_value(b.config.export_all()).unwrap_or(Value::Null))
}

pub async fn import_config(State(b): State<AppState>, Json(body): Json<PersistedConfig>) -> Resp {
    b.config.import_all(body.clone());
    b.events.load_events(body.events.clone());
    ok_msg_only("导入成功")
}

#[cfg(test)]
mod tests {
    use super::SendBody;
    use serde_json::json;

    /// 回归：消息中心前端按 camelCase 发送（serverId/targetType/targetId/messageType），
    /// SendBody 必须能正确反序列化。修复前缺少 #[serde(rename_all = "camelCase")]，
    /// 导致 serverId 等字段全部丢失 → send_via 取不到 server_id → 返回 SERVER_NOT_FOUND，
    /// 消息中心的广播 / 指定客户端发送全部失效。
    #[test]
    fn send_body_deserializes_camel_case() {
        let body = json!({
            "serverId": "srv-1",
            "targetType": "client",
            "targetId": "abc",
            "event": "myEvent",
            "messageType": "json",
            "content": "{\"msg\":\"hi\"}"
        });
        let parsed: SendBody =
            serde_json::from_value(body).expect("SendBody 应能从 camelCase 请求体反序列化");
        assert_eq!(parsed.server_id.as_deref(), Some("srv-1"));
        assert_eq!(parsed.target_type.as_deref(), Some("client"));
        assert_eq!(parsed.target_id.as_deref(), Some("abc"));
        assert_eq!(parsed.event, "myEvent");
        assert_eq!(parsed.message_type.as_deref(), Some("json"));
        assert_eq!(parsed.content.as_deref(), Some("{\"msg\":\"hi\"}"));
    }
}

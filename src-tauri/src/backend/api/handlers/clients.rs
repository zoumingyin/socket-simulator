//! 客户端管理处理函数（/api/clients、/api/client/*、/api/send-message）

use std::collections::HashMap;

use axum::extract::{Json, Query, State};
use axum::http::HeaderMap;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::backend::api::handlers::{audit_log, err, ok, ok_msg_only, Resp};
use crate::backend::constants::CLIENT_ID_SEP;
use crate::backend::state::AppState;

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClientDisconnect {
    client_id: String,
}

#[derive(Deserialize, utoipa::ToSchema)]
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

#[utoipa::path(
    get,
    path = "/api/clients",
    params(
        ("serverId" = Option<String>, Query, description = "按服务 ID 过滤"),
    ),
    responses((status = 200, description = "OK"))
)]
pub async fn get_clients(
    State(b): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> Resp {
    let server_id = q.get("serverId").map(|s| s.as_str());
    let clients = b.clients.get_clients(server_id);
    ok(serde_json::to_value(clients).unwrap_or(Value::Null))
}

#[utoipa::path(
    post,
    path = "/api/client/disconnect",
    request_body = ClientDisconnect,
    responses((status = 200, description = "OK"))
)]
pub async fn client_disconnect(
    State(b): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ClientDisconnect>,
) -> Resp {
    let (server_id, actual) = match body.client_id.split_once(CLIENT_ID_SEP) {
        Some((s, c)) => (s.to_string(), c.to_string()),
        None => (String::new(), body.client_id.clone()),
    };
    match b.services.disconnect_client(&server_id, &actual).await {
        Ok(()) => {
            audit_log(
                &b,
                &headers,
                "client_disconnect",
                "client",
                Some(body.client_id.clone()),
                json!({}),
                true,
            )
            .await;
            ok_msg_only("客户端已断开")
        }
        Err(e) => {
            audit_log(
                &b,
                &headers,
                "client_disconnect",
                "client",
                Some(body.client_id.clone()),
                json!({ "error": e.to_string() }),
                false,
            )
            .await;
            err(e.error_code(), e.to_string(), e.status_code())
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/client/send",
    request_body = SendBody,
    responses((status = 200, description = "OK"))
)]
pub async fn client_send(State(b): State<AppState>, Json(body): Json<SendBody>) -> Resp {
    send_via(b, body).await
}

#[utoipa::path(
    post,
    path = "/api/send-message",
    request_body = SendBody,
    responses((status = 200, description = "OK"))
)]
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

#[cfg(test)]
mod tests {
    use super::{ClientDisconnect, SendBody};
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

    /// 回归：客户端管理前端按 camelCase 发送 `clientId` 断开客户端，
    /// 修复前 ClientDisconnect 缺少 #[serde(rename_all = "camelCase")]，
    /// 导致 client_id 反序列化丢失 → 断开客户端静默失败。
    #[test]
    fn client_disconnect_deserializes_camel_case() {
        let body = json!({ "clientId": "client-xyz" });
        let parsed: ClientDisconnect =
            serde_json::from_value(body).expect("ClientDisconnect 应能从 camelCase 请求体反序列化");
        assert_eq!(parsed.client_id, "client-xyz");
    }
}

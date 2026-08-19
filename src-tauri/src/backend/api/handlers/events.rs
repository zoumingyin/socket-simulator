//! 事件管理处理函数（/api/events/*）

use std::collections::HashMap;

use axum::extract::{Json, Query, State};
use axum::http::HeaderMap;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::backend::api::handlers::{audit_log, err, ok, ok_msg, ok_msg_only, Resp};
use crate::backend::state::AppState;
use crate::backend::types::*;

#[derive(Deserialize, utoipa::ToSchema)]
pub(crate) struct EventId {
    id: String,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub(crate) struct EventToggle {
    id: String,
    status: String,
}

#[utoipa::path(
    get,
    path = "/api/events",
    params(
        ("serverId" = Option<String>, Query, description = "按服务 ID 过滤"),
    ),
    responses((status = 200, description = "OK"))
)]
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

#[utoipa::path(
    post,
    path = "/api/events/add",
    request_body = EventConfig,
    responses((status = 200, description = "OK"))
)]
pub async fn event_add(
    State(b): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<EventConfig>,
) -> Resp {
    let evt = b.events.add_event(body.clone());
    audit_log(
        &b,
        &headers,
        "event_add",
        "event",
        Some(evt.id.clone()),
        json!({ "name": evt.name, "serverId": evt.server_id }),
        true,
    )
    .await;
    ok_msg(
        serde_json::to_value(evt).unwrap_or(Value::Null),
        "事件添加成功",
    )
}

#[utoipa::path(
    post,
    path = "/api/events/update",
    request_body = EventConfig,
    responses((status = 200, description = "OK"))
)]
pub async fn event_update(
    State(b): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<EventConfig>,
) -> Resp {
    match b.events.update_event(&body.id, body.clone()) {
        Some(evt) => {
            audit_log(
                &b,
                &headers,
                "event_update",
                "event",
                Some(evt.id.clone()),
                json!({ "name": evt.name, "serverId": evt.server_id }),
                true,
            )
            .await;
            ok_msg(
                serde_json::to_value(evt).unwrap_or(Value::Null),
                "事件更新成功",
            )
        }
        None => {
            audit_log(
                &b,
                &headers,
                "event_update",
                "event",
                Some(body.id.clone()),
                json!({ "error": "EVENT_NOT_FOUND" }),
                false,
            )
            .await;
            err("EVENT_NOT_FOUND", "事件不存在".into(), 404)
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/events/remove",
    request_body = EventId,
    responses((status = 200, description = "OK"))
)]
pub async fn event_remove(
    State(b): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<EventId>,
) -> Resp {
    if b.events.remove_event(&body.id) {
        audit_log(
            &b,
            &headers,
            "event_remove",
            "event",
            Some(body.id.clone()),
            json!({}),
            true,
        )
        .await;
        ok_msg_only("事件删除成功")
    } else {
        audit_log(
            &b,
            &headers,
            "event_remove",
            "event",
            Some(body.id.clone()),
            json!({ "error": "EVENT_NOT_FOUND" }),
            false,
        )
        .await;
        err("EVENT_NOT_FOUND", "事件不存在".into(), 404)
    }
}

#[utoipa::path(
    post,
    path = "/api/events/toggle",
    request_body = EventToggle,
    responses((status = 200, description = "OK"))
)]
pub async fn event_toggle(
    State(b): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<EventToggle>,
) -> Resp {
    let status = if body.status == "enabled" {
        EventStatus::Enabled
    } else {
        EventStatus::Disabled
    };
    match b.events.toggle_event(&body.id, status) {
        Some(evt) => {
            audit_log(
                &b,
                &headers,
                "event_toggle",
                "event",
                Some(evt.id.clone()),
                json!({ "status": evt.status }),
                true,
            )
            .await;
            ok_msg(
                serde_json::to_value(evt).unwrap_or(Value::Null),
                "状态切换成功",
            )
        }
        None => {
            audit_log(
                &b,
                &headers,
                "event_toggle",
                "event",
                Some(body.id.clone()),
                json!({ "error": "EVENT_NOT_FOUND" }),
                false,
            )
            .await;
            err("EVENT_NOT_FOUND", "事件不存在".into(), 404)
        }
    }
}

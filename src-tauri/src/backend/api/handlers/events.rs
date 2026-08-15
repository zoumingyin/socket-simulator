//! 事件管理处理函数（/api/events/*）

use std::collections::HashMap;

use axum::extract::{Json, Query, State};
use serde::Deserialize;
use serde_json::Value;

use crate::backend::api::handlers::{err, ok, ok_msg, ok_msg_only, Resp};
use crate::backend::state::AppState;
use crate::backend::types::*;

#[derive(Deserialize)]
pub(crate) struct EventId {
    id: String,
}

#[derive(Deserialize)]
pub(crate) struct EventToggle {
    id: String,
    status: String,
}

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

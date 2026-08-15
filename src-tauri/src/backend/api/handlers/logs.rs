//! 日志处理函数（/api/logs、/api/logs/clear）

use std::collections::HashMap;

use axum::extract::{Query, State};
use serde_json::Value;

use crate::backend::api::handlers::{ok, ok_msg_only, Resp};
use crate::backend::state::AppState;
use crate::backend::types::*;

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

//! 日志处理函数（/api/logs、/api/logs/clear）

use std::collections::HashMap;

use axum::extract::{Query, State};
use axum::http::HeaderMap;
use serde_json::{Value, json};

use crate::backend::api::handlers::{audit_log, ok, ok_msg_only, Resp};
use crate::backend::state::AppState;
use crate::backend::types::*;

#[utoipa::path(
    get,
    path = "/api/logs",
    params(
        ("serverId" = Option<String>, Query, description = "按服务 ID 过滤"),
        ("level" = Option<String>, Query, description = "日志级别：DEBUG/INFO/WARN/ERROR"),
        ("keyword" = Option<String>, Query, description = "关键词过滤"),
    ),
    responses((status = 200, description = "OK"))
)]
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

#[utoipa::path(
    post,
    path = "/api/logs/clear",
    responses((status = 200, description = "OK"))
)]
pub async fn logs_clear(State(b): State<AppState>, headers: HeaderMap) -> Resp {
    b.logs.clear_entries();
    audit_log(&b, &headers, "logs_clear", "log", None, json!({}), true).await;
    ok_msg_only("日志已清空")
}

/// 历史持久化日志分页查询（P1-4）：SQLite 全量 + 过滤 + 分页
#[utoipa::path(
    get,
    path = "/api/logs/persisted",
    params(
        ("limit" = Option<i64>, Query, description = "每页条数（默认 100，最大 1000）"),
        ("offset" = Option<i64>, Query, description = "偏移量"),
        ("serverId" = Option<String>, Query, description = "按服务 ID 过滤"),
        ("level" = Option<String>, Query, description = "日志级别下限：DEBUG/INFO/WARN/ERROR"),
        ("keyword" = Option<String>, Query, description = "关键词过滤"),
    ),
    responses((status = 200, description = "返回 { total, items } 分页结构"))
)]
pub async fn get_logs_persisted(
    State(b): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> Resp {
    let limit = q
        .get("limit")
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(100)
        .clamp(1, 1000);
    let offset = q
        .get("offset")
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0)
        .max(0);
    let level = q
        .get("level")
        .and_then(|l| serde_json::from_value::<LogLevel>(Value::String(l.clone())).ok());
    let filter = LogFilter {
        server_id: q.get("serverId").cloned(),
        level,
        keyword: q.get("keyword").cloned(),
        ..Default::default()
    };
    let (total, items) = b.logs.query_persisted(&filter, limit, offset);
    ok(json!({ "total": total, "items": items }))
}

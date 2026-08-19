//! 审计日志查询处理函数（/api/audit/logs）

use axum::extract::{Query, State};
use serde_json::Value;

use crate::backend::api::handlers::{ok, Resp};
use crate::backend::audit::AuditQuery;
use crate::backend::state::AppState;

#[utoipa::path(
    get,
    path = "/api/audit/logs",
    responses((status = 200, description = "OK"))
)]
pub async fn audit_logs(
    State(b): State<AppState>,
    Query(q): Query<AuditQuery>,
) -> Resp {
    match b.audit.query(&q).await {
        Ok(page) => ok(serde_json::to_value(page).unwrap_or(Value::Null)),
        Err(e) => crate::backend::api::handlers::err(
            "AUDIT_QUERY_FAILED",
            e.to_string(),
            500,
        ),
    }
}

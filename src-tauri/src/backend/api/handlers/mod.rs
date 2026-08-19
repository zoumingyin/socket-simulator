//! REST API 处理函数（≡ Node `backend/api/index.ts`）
//!
//! 按域拆分为子模块：mock / servers / events / clients / logs / settings / config。
//! 本模块仅承载共享的响应辅助（`Resp` / `ok` / `err` ...）与子模块的再导出，
//! 使 `handlers::<fn>` 路径（被 `api/router.rs` 使用）保持不变。
//!
//! 每个处理函数统一返回 `(StatusCode, Json<ApiResponse<Value>>)`，
//! 与现网 `{ success, data?, errorCode?, error?, message?, timestamp }` 契约一致。

use axum::http::HeaderMap;
use axum::http::StatusCode;
use serde_json::Value;

use crate::backend::app::Backend;
use crate::backend::audit::record_audit;
use crate::backend::types::{ApiResponse, now_rfc3339};

pub mod audit;
pub mod clients;
pub mod config;
pub mod events;
pub mod logs;
pub mod mock;
pub mod servers;
pub mod settings;

pub use audit::*;
pub use clients::*;
pub use config::*;
pub use events::*;
pub use logs::*;
pub use mock::*;
pub use servers::*;
pub use settings::*;

type Resp = (StatusCode, axum::Json<ApiResponse<Value>>);

/// 关键操作审计埋点（P0-3）：成功/失败均记录；审计为旁路，失败不阻断业务。
/// actor 从请求头 Bearer token 解析（鉴权关闭时视为 admin）。
pub(crate) async fn audit_log(
    b: &Backend,
    headers: &HeaderMap,
    action: &str,
    target_type: &str,
    target_id: Option<String>,
    detail: Value,
    success: bool,
) {
    let role = crate::backend::audit::actor_role(b, headers).await;
    record_audit(b, role, action, target_type, target_id, detail, success).await;
}

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

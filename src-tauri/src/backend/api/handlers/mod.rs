//! REST API 处理函数（≡ Node `backend/api/index.ts`）
//!
//! 按域拆分为子模块：mock / servers / events / clients / logs / settings / config。
//! 本模块仅承载共享的响应辅助（`Resp` / `ok` / `err` ...）与子模块的再导出，
//! 使 `handlers::<fn>` 路径（被 `api/router.rs` 使用）保持不变。
//!
//! 每个处理函数统一返回 `(StatusCode, Json<ApiResponse<Value>>)`，
//! 与现网 `{ success, data?, errorCode?, error?, message?, timestamp }` 契约一致。

use axum::http::StatusCode;
use serde_json::Value;

use crate::backend::types::{ApiResponse, now_rfc3339};

pub mod clients;
pub mod config;
pub mod events;
pub mod logs;
pub mod mock;
pub mod servers;
pub mod settings;

pub use clients::*;
pub use config::*;
pub use events::*;
pub use logs::*;
pub use mock::*;
pub use servers::*;
pub use settings::*;

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

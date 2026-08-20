//! responder.rs —— 根据 MockRule/默认响应构造 axum Response
//!
//! 仅负责「构造响应体/状态码/响应头」；**延迟由 `engine::dispatch` 统一
//! `tokio::time::sleep` 生效**（唯一权威入口，覆盖独立端口/自定义端口/共端口全部路径）。

use axum::body::Body;
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, Response, StatusCode};
use axum::response::IntoResponse;

use crate::backend::types::{MockMatchCondition, MockServiceConfig};

/// 尝试将 JSON 字符串 pretty-print；若解析失败则原样返回
fn pretty_json(raw: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(v) => serde_json::to_string_pretty(&v).unwrap_or_else(|_| raw.to_string()),
        Err(_) => raw.to_string(),
    }
}

/// 构造 JSON Content-Type 头
fn json_content_type() -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    h
}

/// 默认响应（未匹配规则）
pub fn default_response(cfg: &MockServiceConfig) -> Response<Body> {
    let body = pretty_json(&cfg.default_response_body);
    (
        StatusCode::from_u16(cfg.default_status_code).unwrap_or(StatusCode::OK),
        json_content_type(),
        Body::from(body),
    )
        .into_response()
}

/// 规则命中响应
pub fn rule_response(
    rule_status: u16,
    rule_headers: &[MockMatchCondition],
    rule_body: &str,
) -> Response<Body> {
    let status = StatusCode::from_u16(rule_status).unwrap_or(StatusCode::OK);
    let mut headers = json_content_type();
    for h in rule_headers {
        if !h.enabled || h.key.is_empty() {
            continue;
        }
        if let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(h.key.as_bytes()),
            HeaderValue::from_str(&h.value),
        ) {
            headers.insert(name, value);
        }
    }
    let body = pretty_json(rule_body);
    (status, headers, Body::from(body)).into_response()
}

/// JSON 格式错误响应（用于 403/404 等非业务错误）
pub fn json_error_response(status: StatusCode, error: &str, message: &str) -> Response<Body> {
    let body = serde_json::json!({
        "error": error,
        "message": message,
        "status": status.as_u16(),
    });
    let body = serde_json::to_string_pretty(&body).unwrap_or_else(|_| body.to_string());
    (status, json_content_type(), Body::from(body)).into_response()
}

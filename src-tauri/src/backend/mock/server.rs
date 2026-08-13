//! server.rs —— Mock 服务路由分发核心（axum Router + 纯函数 dispatch）
//!
//! 两种使用方式：
//! - 独立端口：build_router + start_custom_port（每个 mock 服务一个 TcpListener）
//! - 主端口挂载：MockManager::dispatch_main_port 由 AppState 调用，命中 basePath 前缀即转交

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::body::{to_bytes, Body};
use axum::extract::connect_info::ConnectInfo;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::{from_fn_with_state, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use axum::Router;
use tower_http::cors::CorsLayer;

use crate::backend::constants::*;
use crate::backend::net::port_release::release_port;
use crate::backend::types::{MockServiceConfig, SystemSettings};

use super::matcher::match_rule;
use super::responder::{default_response, json_error_response, rule_response};

const MAX_BODY_SIZE: usize = 16 * 1024 * 1024; // 16MB

/// 共享状态
#[derive(Clone)]
pub struct MockAppState {
    pub cfg: Arc<MockServiceConfig>,
    pub sys: SystemSettings,
}

impl MockAppState {
    fn allow_ip(&self, ip: &str) -> bool {
        let wl = &self.sys.ip_access.whitelist;
        let bl = &self.sys.ip_access.blacklist;
        if bl.iter().any(|b| b == ip) {
            return false;
        }
        if !wl.is_empty() && !wl.iter().any(|w| w == ip) {
            return false;
        }
        true
    }
}

/// Mock 服务构建/启动
pub struct MockServer;

impl MockServer {
    /// 构造 axum Router（含 IP 过滤中间件）
    pub fn build_router(cfg: MockServiceConfig, sys: SystemSettings) -> Router {
        let state = MockAppState {
            cfg: Arc::new(cfg),
            sys,
        };
        let ip_state = state.clone();
        Router::new()
            .fallback(any(mock_handler))
            .layer(from_fn_with_state(ip_state, ip_guard))
            .layer(CorsLayer::permissive())
            .with_state(state)
    }

    /// 自定义端口启动
    pub async fn start_custom_port(
        cfg: MockServiceConfig,
        sys: SystemSettings,
        port: u16,
    ) -> Result<tokio::sync::oneshot::Sender<()>, String> {
        let addr = ("127.0.0.1", port);
        let listener = match tokio::net::TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                eprintln!("[MockServer] 端口 {} 被占用，尝试释放后重试", port);
                release_port(port);
                tokio::time::sleep(Duration::from_millis(PORT_RELEASE_RETRY_DELAY_MS)).await;
                tokio::net::TcpListener::bind(addr)
                    .await
                    .map_err(|e| format!("端口 {} 绑定失败: {}", port, e))?
            }
            Err(e) => return Err(format!("端口 {} 绑定失败: {}", port, e)),
        };
        let app = Self::build_router(cfg, sys).into_make_service_with_connect_info::<SocketAddr>();
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        tokio::spawn(async move {
            let shutdown = async {
                let _ = rx.await;
            };
            if let Err(e) = axum::serve(listener, app).with_graceful_shutdown(shutdown).await {
                eprintln!("[MockServer] serve error: {}", e);
            }
        });
        Ok(tx)
    }
}

/// IP 黑白名单中间件
async fn ip_guard(
    State(state): State<MockAppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request,
    next: Next,
) -> Response {
    if !state.allow_ip(&addr.ip().to_string()) {
        return json_error_response(
            StatusCode::FORBIDDEN,
            "Forbidden",
            &format!("IP address {} is not allowed", addr.ip()),
        );
    }
    next.run(req).await
}

/// 独立端口 handler（catch-all）
async fn mock_handler(State(state): State<MockAppState>, req: Request) -> Response {
    let cfg = state.cfg.as_ref();
    let sys = state.sys.clone();
    dispatch(cfg, &sys, req).await
}

/// 纯函数 dispatch：剥离 base_path → 规则匹配 → 响应
pub async fn dispatch(cfg: &MockServiceConfig, sys: &SystemSettings, req: Request) -> Response {
    // IP 黑白名单检查
    if let Some(addr) = req.extensions().get::<ConnectInfo<SocketAddr>>().cloned() {
        if !allow_ip_simple(&sys.ip_access.whitelist, &sys.ip_access.blacklist, &addr.ip().to_string()) {
            return json_error_response(
                StatusCode::FORBIDDEN,
                "Forbidden",
                &format!("IP address {} is not allowed", addr.ip()),
            );
        }
    }

    // 在 into_parts 前先把只读数据拷出来
    let method = req.method().as_str().to_string();
    let full_path = req.uri().path().to_string();
    let query_q = req.uri().query().map(|s| s.to_string());

    let relative = strip_base(&full_path, &cfg.base_path);
    let query_map = parse_query(query_q.as_deref());

    let (parts, body) = req.into_parts();
    let body_bytes = to_bytes(body, MAX_BODY_SIZE).await.unwrap_or_default();

    // 按规则顺序匹配
    for rule in &cfg.rules {
        if match_rule(rule, &method, &relative, &parts.headers, &query_map, &body_bytes).is_some() {
            if rule.response_delay_ms > 0 {
                tokio::time::sleep(Duration::from_millis(rule.response_delay_ms as u64)).await;
            }
            return rule_response(
                rule.response_status_code,
                &rule.response_headers,
                &rule.response_body,
                rule.response_delay_ms,
            );
        }
    }

    if cfg.default_delay_ms > 0 {
        tokio::time::sleep(Duration::from_millis(cfg.default_delay_ms as u64)).await;
    }
    default_response(cfg)
}

fn allow_ip_simple(wl: &[String], bl: &[String], ip: &str) -> bool {
    if bl.iter().any(|b| b == ip) {
        return false;
    }
    if !wl.is_empty() && !wl.iter().any(|w| w == ip) {
        return false;
    }
    true
}

fn strip_base(full: &str, base: &str) -> String {
    let base = base.trim_end_matches('/');
    if base.is_empty() {
        return full.to_string();
    }
    if full == base {
        return "/".to_string();
    }
    if let Some(rest) = full.strip_prefix(base) {
        if rest.is_empty() || rest.starts_with('/') {
            return rest.to_string();
        }
    }
    full.to_string()
}

fn parse_query(q: Option<&str>) -> serde_json::Map<String, serde_json::Value> {
    let mut m = serde_json::Map::new();
    if let Some(s) = q {
        for pair in s.split('&') {
            if pair.is_empty() {
                continue;
            }
            let (k, v) = match pair.split_once('=') {
                Some((k, v)) => (k, url_decode(v)),
                None => (pair, String::new()),
            };
            m.insert(url_decode(k), serde_json::Value::String(v));
        }
    }
    m
}

fn url_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'+' {
            out.push(b' ');
            i += 1;
        } else if b == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push((h << 4) | l);
                i += 3;
            } else {
                out.push(b);
                i += 1;
            }
        } else {
            out.push(b);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

// 抑制未使用导入警告
#[allow(dead_code)]
fn _keep_link() -> Body { Body::empty() }
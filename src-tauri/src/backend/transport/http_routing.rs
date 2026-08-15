//! http_routing —— `HttpServer` 与 `UnifiedServer` 共享的 HTTP 路由构建与处理器
//!
//! 两者的 HTTP 入站 / 流 / SSE / 路由构建逻辑完全一致，仅注入的状态字段来源不同。
//! 通过 `HttpRouteState` trait 抽象状态，将以下重复实现收敛为单一实现：
//!
//! - `to_axum_path` / `default_http_routes`（路径占位符与默认路由）
//! - `build_http_router`（inbound/stream 分组注册 + IP 中间件）
//! - `ingress_handler` / `stream_handler` / `ip_guard`（请求处理器与中间件）
//! - `ClientStream`（SSE 流 + 断开清理）
//! - `parse_query` / `url_decode` / `hex`（查询串解析，Mock 引擎也复用）
//!
//! 用法：实现 `HttpRouteState` 的 struct 调用 `build_http_router(effective, state.clone())`
//! 后再自行追加 `.fallback(...)` / CORS 等并 `.with_state(state)`。

use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

use axum::body::to_bytes;
use axum::extract::connect_info::ConnectInfo;
use axum::extract::{MatchedPath, Path, Request, State};
use axum::http::StatusCode;
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::MethodRouter;
use axum::middleware::Next;
use axum::Router;
use futures_util::Stream;
use nanoid::nanoid;
use serde_json::Value;
use tokio::sync::mpsc;

use crate::backend::constants::*;
use crate::backend::transport::hooks::TransportHooks;
use crate::backend::types::*;

/// SSE 客户端外发消息发送端（HttpServer / UnifiedServer 共用）
pub type SseTx = mpsc::UnboundedSender<SseEvent>;

/// HTTP 路由状态抽象：HttpServer 与 UnifiedServer 各自实现，共享通用处理器
pub trait HttpRouteState: Clone + Send + Sync + 'static {
    fn hooks(&self) -> &TransportHooks;
    fn routes(&self) -> &Arc<HashMap<String, HttpRouteConfig>>;
    fn server_id(&self) -> &str;
    fn protocol(&self) -> ProtocolType;
    fn sse_clients(&self) -> &Arc<Mutex<HashMap<String, SseTx>>>;
    fn allow_ip(&self, ip: &str) -> bool;
    /// IP 被拒时的响应（HttpServer 返回纯 403，Unified 返回 JSON 403）
    /// `ip` 为来源 IP，便于 Unified 在响应中回显
    fn ip_denied(&self, ip: &str) -> Response;
}

/// 把用户路径中的 `{name}` 占位符转换为 axum 的 `:name` 语法
pub fn to_axum_path(p: &str) -> String {
    p.split('/')
        .map(|seg| {
            if seg.starts_with('{') && seg.ends_with('}') && seg.len() > 2 {
                format!(":{}", &seg[1..seg.len() - 1])
            } else {
                seg.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// 内置默认 HTTP transport 路由（http_routes 为空时使用）
pub fn default_http_routes() -> Vec<HttpRouteConfig> {
    vec![
        HttpRouteConfig {
            id: "__default_inbound".to_string(),
            method: HttpMethod::Post,
            path: "/{event}".to_string(),
            route_type: HttpRouteType::Inbound,
            event: None,
            description: Some("默认入站 POST /{event}".to_string()),
        },
        HttpRouteConfig {
            id: "__default_stream".to_string(),
            method: HttpMethod::Get,
            path: "/stream".to_string(),
            route_type: HttpRouteType::Stream,
            event: None,
            description: Some("默认 SSE 推送 GET /stream".to_string()),
        },
    ]
}

/// 构建路由查找表：key = `"METHOD /axum/path"`，handler 用 MatchedPath + method 查回 route 配置（取固定 event 名等）
pub fn build_routes_map(effective: &[HttpRouteConfig]) -> HashMap<String, HttpRouteConfig> {
    let mut m = HashMap::new();
    for r in effective {
        let key = format!("{} {}", r.method.as_str(), to_axum_path(&r.path));
        m.insert(key, r.clone());
    }
    m
}

/// 构建 HTTP transport 路由（inbound/stream 分组注册 + IP 中间件）
///
/// 返回未烘焙状态的 `Router<S>`；调用方需自行追加 `.fallback(...)` / CORS 并 `.with_state(state)`。
pub fn build_http_router<S: HttpRouteState>(
    effective: Vec<HttpRouteConfig>,
    _state: S,
) -> Router<S> {
    // IP 中间件由调用方在 build_http_router 之后自行追加（unified 需覆盖 fallback）

    // 按 axum 路径分组：inbound / stream 分别收集，避免同路径不同 handler 冲突
    // 同一 (path, method) 重复时去重，防止 axum panic
    let mut inbound_by_path: HashMap<String, Vec<HttpMethod>> = HashMap::new();
    let mut stream_by_path: HashMap<String, Vec<HttpMethod>> = HashMap::new();
    for r in &effective {
        let axum_path = to_axum_path(&r.path);
        let bucket = if r.route_type == HttpRouteType::Stream {
            &mut stream_by_path
        } else {
            &mut inbound_by_path
        };
        let v = bucket.entry(axum_path).or_default();
        if !v.contains(&r.method) {
            v.push(r.method);
        }
    }

    let mut router: Router<S> = Router::new();
    for (path, methods) in inbound_by_path {
        let mut mr: MethodRouter<S> = MethodRouter::new();
        for m in methods {
            mr = match m {
                HttpMethod::Get => mr.get(ingress_handler::<S>),
                HttpMethod::Post => mr.post(ingress_handler::<S>),
                HttpMethod::Put => mr.put(ingress_handler::<S>),
                HttpMethod::Delete => mr.delete(ingress_handler::<S>),
                HttpMethod::Patch => mr.patch(ingress_handler::<S>),
                HttpMethod::Head => mr.head(ingress_handler::<S>),
                HttpMethod::Options => mr.options(ingress_handler::<S>),
                HttpMethod::Any => continue,
            };
        }
        router = router.route(&path, mr);
    }
    for (path, methods) in stream_by_path {
        let mut mr: MethodRouter<S> = MethodRouter::new();
        for m in methods {
            mr = match m {
                HttpMethod::Get => mr.get(stream_handler::<S>),
                HttpMethod::Post => mr.post(stream_handler::<S>),
                HttpMethod::Put => mr.put(stream_handler::<S>),
                HttpMethod::Delete => mr.delete(stream_handler::<S>),
                HttpMethod::Patch => mr.patch(stream_handler::<S>),
                HttpMethod::Head => mr.head(stream_handler::<S>),
                HttpMethod::Options => mr.options(stream_handler::<S>),
                HttpMethod::Any => continue,
            };
        }
        router = router.route(&path, mr);
    }

    router
}

/// 入站消息处理器（与 HttpServer.ingress_handler 一致）
///
/// 事件名优先级：route.event > 路径 {event} 参数 > 路径末段 > "message"
pub async fn ingress_handler<S: HttpRouteState>(
    State(state): State<S>,
    params: Path<HashMap<String, String>>,
    req: Request,
) -> Response {
    let method = req.method().as_str().to_uppercase();
    let matched = req
        .extensions()
        .get::<MatchedPath>()
        .map(|m| m.as_str().to_string());
    let route = matched
        .as_ref()
        .and_then(|p| state.routes().get(&format!("{} {}", method, p)));

    let event = route
        .and_then(|r| r.event.clone())
        .or_else(|| params.get("event").cloned())
        .or_else(|| {
            req.uri()
                .path()
                .trim_end_matches('/')
                .rsplit('/')
                .next()
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "message".to_string());

    let data: Value = match to_bytes(req.into_body(), usize::MAX).await {
        Ok(b) if !b.is_empty() => serde_json::from_slice(&b)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&b).to_string())),
        _ => Value::Null,
    };
    // HTTP 入站无持久客户端，来源记为 http-ingress（仅用于日志）
    (state.hooks().on_message)("http-ingress".to_string(), event, data);
    (
        StatusCode::OK,
        axum::Json(serde_json::json!({ "ok": true })),
    )
        .into_response()
}

/// 实时推送处理器（与 HttpServer.stream_handler 一致）
pub async fn stream_handler<S: HttpRouteState>(
    State(state): State<S>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Sse<ClientStream> {
    let cid = nanoid!(16);
    let (tx, rx) = mpsc::unbounded_channel::<SseEvent>();
    state.sse_clients().lock().unwrap().insert(cid.clone(), tx);

    // 通知连接（与 WsServer 一致的客户端信息）
    let now = now_rfc3339();
    let info = ClientInfo {
        id: cid.clone(),
        server_id: state.server_id().to_string(),
        socket_id: cid.clone(),
        ip_address: addr.ip().to_string(),
        connected_at: now.clone(),
        last_activity_at: now,
        protocol: state.protocol(),
        status: ClientStatus::Connected,
        ..Default::default()
    };
    (state.hooks().on_connect)(info);

    let stream = ClientStream {
        rx,
        cid: cid.clone(),
        sse_clients: state.sse_clients().clone(),
        hooks: state.hooks().clone(),
    };
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

/// IP 黑白名单中间件（拒绝则返回状态实现的 ip_denied 响应）
pub async fn ip_guard<S: HttpRouteState>(
    State(state): State<S>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request,
    next: Next,
) -> Response {
    let ip = addr.ip().to_string();
    if !state.allow_ip(&ip) {
        return state.ip_denied(&ip);
    }
    next.run(req).await
}

/// SSE 流包装：当连接断开（流被 drop）时，从客户端表移除并回调 on_disconnect
pub struct ClientStream {
    rx: mpsc::UnboundedReceiver<SseEvent>,
    cid: String,
    sse_clients: Arc<Mutex<HashMap<String, SseTx>>>,
    hooks: TransportHooks,
}

impl Stream for ClientStream {
    type Item = Result<SseEvent, Infallible>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        match this.rx.poll_recv(cx) {
            Poll::Ready(Some(e)) => Poll::Ready(Some(Ok(e))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl Drop for ClientStream {
    fn drop(&mut self) {
        self.sse_clients.lock().unwrap().remove(&self.cid);
        (self.hooks.on_disconnect)(self.cid.clone());
    }
}

// ==================== 查询串解析（Mock 引擎与 Unified 共用） ====================

/// 解析 URL query 串为有序 map（保留顺序）
pub fn parse_query(q: Option<&str>) -> serde_json::Map<String, serde_json::Value> {
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

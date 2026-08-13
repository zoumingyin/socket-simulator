//! UnifiedServer —— 统一路由传输层（Mock HTTP + Socket 共端口协同）
//!
//! ## 设计目标
//! 将 Mock HTTP 服务和 Socket（WebSocket/HTTP）服务合并到同一个 axum Router 中，
//! 使两者能够在统一的入口下协同工作。通过请求类型自动区分：
//!
//! - **WebSocket 升级请求**（含 `Upgrade: websocket` 头）→ 由 Socket 传输层处理
//!   （建立 WS 连接、收发消息、生命周期管理）
//! - **普通 HTTP 请求**（非 WS 升级）→ 由 Mock 引擎处理
//!   （规则匹配 → 返回预设模拟响应）
//!
//! ## 路由优先级
//! 1. 显式注册的 HTTP transport 路由（inbound/stream，仅 protocol=http 时）
//! 2. Fallback 统一处理器（WS 升级检测 → Mock dispatch）
//!
//! ## 互不干扰保证
//! - WebSocket 连接的 error/timeout/disconnect 逻辑完全独立（与 WsServer 一致）
//! - Mock HTTP 的 error/delay 逻辑完全独立（与 MockServer dispatch 一致）
//! - 两者在同一个 Router 的不同分支中执行，不共享状态
//!
//! ## 配置项（ServerConfig 中的 mock_* 字段）
//! - `mock_enabled: bool` — 是否启用统一路由模式
//! - `mock_rules: Vec<MockRule>` — Mock 规则列表
//! - `mock_default_status_code: u16` — 未匹配规则时的默认状态码
//! - `mock_default_response_body: String` — 未匹配规则时的默认响应体
//! - `mock_default_delay_ms: u32` — 未匹配规则时的默认延迟（ms）
//!
//! ## 不支持的协议
//! Socket.IO 使用 hyper 1.0 原生服务，无法直接集成到 axum 0.7 Router 中。
//! 当 `protocol=socket.io` 时，即使 `mock_enabled=true` 也仅启动 Socket.IO 传输层，
//! Mock HTTP 不生效（日志会打印警告）。

use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::task::{Context, Poll};
use std::time::Duration;

use axum::body::to_bytes;
use axum::extract::connect_info::ConnectInfo;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{MatchedPath, Path, Request, State};
use axum::http::StatusCode;
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::MethodRouter;
use axum::middleware::{from_fn_with_state, Next};
use axum::Router;
use futures_util::{SinkExt, Stream, StreamExt};
use serde_json::Value;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, Notify};
// 注意：使用 axum::extract::ws::Message（而非 tokio_tungstenite::tungstenite::Message）

use crate::backend::constants::*;
use crate::backend::error::BackendError;
use crate::backend::mock::matcher::match_rule;
use crate::backend::mock::responder::{
    default_response, json_error_response, rule_response,
};
use crate::backend::net::port_release::release_port;
use crate::backend::transport::Transport;
use crate::backend::transport::websocket::TransportHooks;
use crate::backend::types::*;

use nanoid::nanoid;
use tower_http::cors::CorsLayer;

const MAX_BODY_SIZE: usize = 16 * 1024 * 1024; // 16MB

/// SSE 客户端外发消息发送端（复用 HttpServer 的设计）
type SseTx = mpsc::UnboundedSender<SseEvent>;

/// 统一路由共享状态
///
/// 同时承载 Socket 传输层和 Mock HTTP 引擎所需的状态。
/// 两者在 fallback handler 中通过请求类型自动分发，互不共享可变状态。
#[derive(Clone)]
pub struct UnifiedState {
    // ---------- Socket 传输层状态 ----------
    /// WS 客户端表：raw socketId → 外发消息发送端
    pub ws_clients: Arc<Mutex<HashMap<String, mpsc::Sender<Message>>>>,
    /// SSE 客户端表（HTTP transport stream 模式）：clientId → SSE 发送端
    pub sse_clients: Arc<Mutex<HashMap<String, SseTx>>>,
    /// 传输层回调（连接/消息/断开）
    pub hooks: TransportHooks,
    /// 服务配置（协议、端口、HTTP 路由等）
    pub cfg: ServerConfig,
    /// 系统设置（IP 黑白名单等）
    pub sys: SystemSettings,
    /// 路由配置映射（HTTP transport 用），key = `"METHOD /axum/path"`
    pub routes: Arc<HashMap<String, HttpRouteConfig>>,

    // ---------- Mock HTTP 引擎状态 ----------
    /// 是否启用 Mock HTTP（从 cfg.mock_enabled 拷出，方便 handler 访问）
    pub mock_enabled: bool,
    /// Mock 规则（从 cfg.mock_rules 拷出）
    pub mock_rules: Arc<Vec<MockRule>>,
    /// Mock 默认状态码
    pub mock_default_status_code: u16,
    /// Mock 默认响应体
    pub mock_default_response_body: String,
    /// Mock 默认延迟
    pub mock_default_delay_ms: u32,
}

impl UnifiedState {
    /// IP 黑白名单过滤（与 WsServer/HttpServer 一致）
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

/// 统一路由传输层服务端
///
/// 在单个端口上同时提供：
/// - WebSocket 连接（协议升级请求自动识别）
/// - HTTP transport 路由（inbound 收消息 / stream SSE 推送）
/// - Mock HTTP 响应（普通 HTTP 请求 → 规则匹配 → 模拟响应）
///
/// 三者通过 axum Router 的路由优先级和 fallback handler 自动区分：
/// 1. 显式 HTTP transport 路由优先匹配
/// 2. 未匹配的请求进入 fallback：检测 WS 升级头 → WS 或 Mock
pub struct UnifiedServer {
    cfg: ServerConfig,
    sys: SystemSettings,
    hooks: TransportHooks,
    ws_clients: Arc<Mutex<HashMap<String, mpsc::Sender<Message>>>>,
    sse_clients: Arc<Mutex<HashMap<String, SseTx>>>,
    running: Arc<AtomicBool>,
    shutdown: Mutex<Option<Arc<Notify>>>,
    /// 自引用弱指针（供 WS 连接处理任务获取 Arc<Self>）
    self_ref: Weak<UnifiedServer>,
}

impl UnifiedServer {
    /// 构造（通过 `Arc::new_cyclic` 注入 self 弱引用）
    pub fn new(
        cfg: ServerConfig,
        sys: SystemSettings,
        hooks: TransportHooks,
        weak: Weak<UnifiedServer>,
    ) -> Self {
        Self {
            cfg,
            sys,
            hooks,
            ws_clients: Arc::new(Mutex::new(HashMap::new())),
            sse_clients: Arc::new(Mutex::new(HashMap::new())),
            running: Arc::new(AtomicBool::new(false)),
            shutdown: Mutex::new(None),
            self_ref: weak,
        }
    }

    /// 把用户路径中的 `{name}` 占位符转换为 axum 的 `:name` 语法
    fn to_axum_path(p: &str) -> String {
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
    fn default_http_routes() -> Vec<HttpRouteConfig> {
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

    /// 构建统一路由
    ///
    /// 路由结构（按优先级从高到低）：
    /// 1. HTTP transport 显式路由（inbound/stream，仅 protocol=http 时注册）
    /// 2. Fallback → `unified_fallback`（检测 WS 升级 → WS handler；否则 → Mock dispatch）
    /// 3. IP 黑白名单中间件（最外层）
    /// 4. CORS（最外层）
    fn build_router(&self) -> Router {
        // 选生效 HTTP transport 路由：仅 http 协议时注册
        let effective_http_routes: Vec<HttpRouteConfig> =
            if self.cfg.protocol == ProtocolType::Http {
                if self.cfg.http_routes.is_empty() {
                    Self::default_http_routes()
                } else {
                    self.cfg.http_routes.clone()
                }
            } else {
                Vec::new()
            };

        // routes 映射：key = "METHOD /axum/path"
        let mut routes_map: HashMap<String, HttpRouteConfig> = HashMap::new();
        for r in &effective_http_routes {
            let key = format!("{} {}", r.method.as_str(), Self::to_axum_path(&r.path));
            routes_map.insert(key, r.clone());
        }

        let state = UnifiedState {
            ws_clients: self.ws_clients.clone(),
            sse_clients: self.sse_clients.clone(),
            hooks: self.hooks.clone(),
            cfg: self.cfg.clone(),
            sys: self.sys.clone(),
            routes: Arc::new(routes_map),
            mock_enabled: self.cfg.mock_enabled,
            mock_rules: Arc::new(self.cfg.mock_rules.clone()),
            mock_default_status_code: self.cfg.mock_default_status_code,
            mock_default_response_body: self.cfg.mock_default_response_body.clone(),
            mock_default_delay_ms: self.cfg.mock_default_delay_ms,
        };
        let ip_state = state.clone();

        // 按路径分组注册 HTTP transport 路由
        let mut inbound_by_path: HashMap<String, Vec<HttpMethod>> = HashMap::new();
        let mut stream_by_path: HashMap<String, Vec<HttpMethod>> = HashMap::new();
        for r in &effective_http_routes {
            let axum_path = Self::to_axum_path(&r.path);
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

        let mut router: Router<UnifiedState> = Router::new();
        for (path, methods) in inbound_by_path {
            let mut mr: MethodRouter<UnifiedState> = MethodRouter::new();
            for m in methods {
                mr = match m {
                    HttpMethod::Get => mr.get(http_ingress_handler),
                    HttpMethod::Post => mr.post(http_ingress_handler),
                    HttpMethod::Put => mr.put(http_ingress_handler),
                    HttpMethod::Delete => mr.delete(http_ingress_handler),
                    HttpMethod::Patch => mr.patch(http_ingress_handler),
                    HttpMethod::Head => mr.head(http_ingress_handler),
                    HttpMethod::Options => mr.options(http_ingress_handler),
                    HttpMethod::Any => continue,
                };
            }
            router = router.route(&path, mr);
        }
        for (path, methods) in stream_by_path {
            let mut mr: MethodRouter<UnifiedState> = MethodRouter::new();
            for m in methods {
                mr = match m {
                    HttpMethod::Get => mr.get(http_stream_handler),
                    HttpMethod::Post => mr.post(http_stream_handler),
                    HttpMethod::Put => mr.put(http_stream_handler),
                    HttpMethod::Delete => mr.delete(http_stream_handler),
                    HttpMethod::Patch => mr.patch(http_stream_handler),
                    HttpMethod::Head => mr.head(http_stream_handler),
                    HttpMethod::Options => mr.options(http_stream_handler),
                    HttpMethod::Any => continue,
                };
            }
            router = router.route(&path, mr);
        }

        // Fallback：统一处理器（WS 升级 → Socket；否则 → Mock HTTP）
        router
            .fallback(unified_fallback)
            .layer(from_fn_with_state(ip_state, ip_guard))
            .layer(CorsLayer::permissive())
            .with_state(state)
    }
}

// ==================== 统一 Fallback 处理器 ====================

/// 统一 Fallback —— 核心路由分发器
///
/// 在同一路由处理器内同时处理 HTTP 请求和 WebSocket 连接，通过请求类型区分：
///
/// - `Option<WebSocketUpgrade>` 为 `Some(ws)` 时：
///   请求包含 WebSocket 升级头（`Upgrade: websocket`），交由 WS 传输层处理。
///   建立 WS 连接，注册客户端，开始收发消息循环。
///   错误处理/超时/断开逻辑与 WsServer 完全独立。
///
/// - `Option<WebSocketUpgrade>` 为 `None` 时：
///   请求是普通 HTTP 请求，交由 Mock 引擎处理。
///   按规则顺序匹配（路径/方法/headers/query/body），命中则返回预设响应；
///   未命中则返回默认响应。延迟/error 逻辑与 MockServer 完全独立。
async fn unified_fallback(
    ws: Option<WebSocketUpgrade>,
    State(state): State<UnifiedState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request,
) -> Response {
    if let Some(ws) = ws {
        // ===== WebSocket 升级请求 → Socket 传输层 =====
        //
        // 请求包含 `Upgrade: websocket` 头，axum 的 WebSocketUpgrade 提取器自动识别。
        // 升级后进入 WS 消息循环，与 Mock HTTP 完全隔离。
        let ip = addr.ip().to_string();
        let raw_id = nanoid!(16);
        ws.on_upgrade(move |socket| handle_ws_connection(state, socket, raw_id, ip))
    } else {
        // ===== 普通 HTTP 请求 → Mock 引擎 =====
        //
        // 请求不含 WS 升级头，交由 Mock 引擎处理。
        // 如果 mock_enabled=false 或无匹配规则，返回默认响应或 404。
        if state.mock_enabled {
            mock_dispatch(&state, req).await
        } else {
            json_error_response(
                StatusCode::NOT_FOUND,
                "Not Found",
                "No route matched and mock is not enabled",
            )
        }
    }
}

// ==================== WebSocket 连接处理（Socket 传输层） ====================

/// WS 连接处理 —— 与 WsServer.handle_conn 逻辑一致
///
/// 独立的错误处理/超时/断开逻辑：
/// - 读错误 → 记录日志并断开
/// - 写错误 → 断开
/// - 客户端主动关闭 → 清理
/// - 服务端 stop → 清空 clients → 各连接因 out_rx 关闭而结束
async fn handle_ws_connection(
    state: UnifiedState,
    socket: WebSocket,
    raw_id: String,
    ip: String,
) {
    let (mut write, mut read) = socket.split();
    let (out_tx, mut out_rx) = mpsc::channel::<Message>(64);
    state.ws_clients.lock().unwrap().insert(raw_id.clone(), out_tx);

    // 通知连接
    let now = now_rfc3339();
    let info = ClientInfo {
        id: raw_id.clone(),
        server_id: state.cfg.id.clone(),
        socket_id: raw_id.clone(),
        ip_address: ip.clone(),
        connected_at: now.clone(),
        last_activity_at: now,
        protocol: state.cfg.protocol,
        status: ClientStatus::Connected,
        group: None,
        group_name: None,
        metadata: None,
    };
    (state.hooks.on_connect)(info);

    loop {
        tokio::select! {
            incoming = read.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<WsFrame>(&text) {
                            Ok(frame) => (state.hooks.on_message)(raw_id.clone(), frame.event, frame.data),
                            Err(_) => (state.hooks.on_message)(
                                raw_id.clone(),
                                "message".to_string(),
                                serde_json::json!({ "raw": text }),
                            ),
                        }
                    }
                    Some(Ok(Message::Binary(bin))) => {
                        let text = String::from_utf8_lossy(&bin).to_string();
                        (state.hooks.on_message)(
                            raw_id.clone(),
                            "message".to_string(),
                            serde_json::json!({ "raw": text }),
                        );
                    }
                    Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => {
                        // tungstenite 自动回复 ping
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(e)) => {
                        eprintln!("[UnifiedServer] WS 连接 {} 读错误: {}", raw_id, e);
                        break;
                    }
                    _ => {}
                }
            }
            outgoing = out_rx.recv() => {
                match outgoing {
                    Some(msg) => {
                        if write.send(msg).await.is_err() {
                            break;
                        }
                    }
                    None => break,
                }
            }
        }
    }

    // 清理
    state.ws_clients.lock().unwrap().remove(&raw_id);
    (state.hooks.on_disconnect)(raw_id);
}

// ==================== Mock HTTP 引擎（与 MockServer.dispatch 一致） ====================

/// Mock HTTP 分发 —— 纯函数，处理普通 HTTP 请求
///
/// 独立的 error/delay 逻辑：
/// - 规则命中延迟 → tokio::sleep
/// - 默认延迟 → tokio::sleep
/// - 未启用 → 404 JSON 错误
/// - IP 禁止 → 403 JSON 错误
///
/// 与 WS 连接处理完全隔离，不共享可变状态。
async fn mock_dispatch(state: &UnifiedState, req: Request) -> Response {
    // 在 into_parts 前拷出只读数据
    let method = req.method().as_str().to_string();
    let full_path = req.uri().path().to_string();
    let query_q = req.uri().query().map(|s| s.to_string());

    let query_map = parse_query(query_q.as_deref());

    let (parts, body) = req.into_parts();
    let body_bytes = to_bytes(body, MAX_BODY_SIZE).await.unwrap_or_default();

    // 按规则顺序匹配
    for rule in state.mock_rules.iter() {
        if !rule.enabled {
            continue;
        }
        if match_rule(rule, &method, &full_path, &parts.headers, &query_map, &body_bytes)
            .is_some()
        {
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

    // 未匹配规则 → 默认响应
    if state.mock_default_delay_ms > 0 {
        tokio::time::sleep(Duration::from_millis(state.mock_default_delay_ms as u64)).await;
    }

    // 构造临时 MockServiceConfig 以复用 default_response
    let tmp_cfg = MockServiceConfig {
        default_status_code: state.mock_default_status_code,
        default_response_body: state.mock_default_response_body.clone(),
        default_delay_ms: state.mock_default_delay_ms,
        ..Default::default()
    };
    default_response(&tmp_cfg)
}

// ==================== HTTP Transport 路由处理器 ====================

/// HTTP 入站消息处理器（与 HttpServer.ingress_handler 一致）
async fn http_ingress_handler(
    State(state): State<UnifiedState>,
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
        .and_then(|p| state.routes.get(&format!("{} {}", method, p)));

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
    (state.hooks.on_message)("http-ingress".to_string(), event, data);
    (
        StatusCode::OK,
        axum::Json(serde_json::json!({ "ok": true })),
    )
        .into_response()
}

/// HTTP SSE 推送处理器（与 HttpServer.stream_handler 一致）
async fn http_stream_handler(
    State(state): State<UnifiedState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Sse<ClientStream> {
    let cid = nanoid!(16);
    let (tx, rx) = mpsc::unbounded_channel::<SseEvent>();
    state.sse_clients.lock().unwrap().insert(cid.clone(), tx);

    let now = now_rfc3339();
    let info = ClientInfo {
        id: cid.clone(),
        server_id: state.cfg.id.clone(),
        socket_id: cid.clone(),
        ip_address: addr.ip().to_string(),
        connected_at: now.clone(),
        last_activity_at: now,
        protocol: state.cfg.protocol,
        status: ClientStatus::Connected,
        ..Default::default()
    };
    (state.hooks.on_connect)(info);

    let stream = ClientStream {
        rx,
        cid: cid.clone(),
        sse_clients: state.sse_clients.clone(),
        hooks: state.hooks.clone(),
    };
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

// ==================== 中间件 ====================

/// IP 黑白名单中间件（拒绝则返回 403 JSON）
async fn ip_guard(
    State(state): State<UnifiedState>,
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

// ==================== SSE 流（携带断开清理） ====================

struct ClientStream {
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

// ==================== 工具函数 ====================

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

// ==================== Transport 实现 ====================

#[async_trait::async_trait]
impl Transport for UnifiedServer {
    async fn start(&self) -> Result<(), BackendError> {
        if self.running.load(Ordering::SeqCst) {
            return Ok(());
        }

        // Socket.IO 协议不支持统一路由（hyper 1.0 与 axum 0.7 不兼容）
        if self.cfg.protocol == ProtocolType::SocketIo {
            eprintln!(
                "[UnifiedServer] Socket.IO 协议不支持统一路由模式，Mock HTTP 将不生效（服务: {}）",
                self.cfg.id
            );
        }

        if self.cfg.mock_enabled {
            println!(
                "[UnifiedServer] 服务 {} 启用统一路由模式：协议 + Mock HTTP 共端口 {}",
                self.cfg.id,
                self.cfg.port
            );
        }

        let addr = (self.cfg.ip.as_str(), self.cfg.port as u16);
        let listener = match TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                eprintln!(
                    "[UnifiedServer] 端口 {} 被占用，尝试释放后重试",
                    self.cfg.port
                );
                release_port(self.cfg.port as u16);
                tokio::time::sleep(Duration::from_millis(PORT_RELEASE_RETRY_DELAY_MS)).await;
                TcpListener::bind(addr).await?
            }
            Err(e) => return Err(e.into()),
        };

        let app = self
            .build_router()
            .into_make_service_with_connect_info::<SocketAddr>();
        let notify = Arc::new(Notify::new());
        *self.shutdown.lock().unwrap() = Some(notify.clone());
        self.running.store(true, Ordering::SeqCst);

        tokio::spawn(async move {
            let shutdown = async move { notify.notified().await };
            if let Err(e) = axum::serve(listener, app)
                .with_graceful_shutdown(shutdown)
                .await
            {
                eprintln!("[UnifiedServer] serve error: {}", e);
            }
        });
        Ok(())
    }

    async fn stop(&self) -> Result<(), BackendError> {
        self.running.store(false, Ordering::SeqCst);
        if let Some(n) = self.shutdown.lock().unwrap().take() {
            n.notify_one();
        }
        // 清空 WS 客户端 → 各连接任务因 out_rx 关闭而结束
        self.ws_clients.lock().unwrap().clear();
        // 清空 SSE 客户端 → 各流 drop → on_disconnect
        self.sse_clients.lock().unwrap().clear();
        Ok(())
    }

    async fn send(&self, client_id: &str, event: &str, data: Value) -> Result<(), BackendError> {
        // 优先尝试 WS 客户端
        let ws_msg = Message::Text(
            serde_json::to_string(&WsFrame {
                event: event.to_string(),
                data: data.clone(),
            })
            .unwrap_or_default(),
        );
        let ws_tx = self.ws_clients.lock().unwrap().get(client_id).cloned();
        if let Some(tx) = ws_tx {
            let _ = tx.send(ws_msg).await;
            return Ok(());
        }

        // 尝试 SSE 客户端
        let sse_ev = SseEvent::default()
            .event(event.to_string())
            .data(serde_json::to_string(&data).unwrap_or_default());
        let sse_tx = self.sse_clients.lock().unwrap().get(client_id).cloned();
        if let Some(tx) = sse_tx {
            let _ = tx.send(sse_ev);
        }
        Ok(())
    }

    async fn broadcast(
        &self,
        event: &str,
        data: Value,
        target_ids: Option<Vec<String>>,
    ) -> Result<(), BackendError> {
        // WS 广播
        let ws_payload = Message::Text(
            serde_json::to_string(&WsFrame {
                event: event.to_string(),
                data: data.clone(),
            })
            .unwrap_or_default(),
        );
        let sse_ev = SseEvent::default()
            .event(event.to_string())
            .data(serde_json::to_string(&data).unwrap_or_default());

        let ws_targets: Vec<String> = match &target_ids {
            Some(ids) => ids.clone(),
            None => self.ws_clients.lock().unwrap().keys().cloned().collect(),
        };
        for id in &ws_targets {
            let tx = self.ws_clients.lock().unwrap().get(id).cloned();
            if let Some(tx) = tx {
                let _ = tx.send(ws_payload.clone()).await;
            }
        }

        // SSE 广播
        let sse_targets: Vec<String> = match &target_ids {
            Some(ids) => ids.clone(),
            None => self.sse_clients.lock().unwrap().keys().cloned().collect(),
        };
        for id in &sse_targets {
            let tx = self.sse_clients.lock().unwrap().get(id).cloned();
            if let Some(tx) = tx {
                let _ = tx.send(sse_ev.clone());
            }
        }
        Ok(())
    }

    async fn disconnect_client(&self, client_id: &str) -> Result<(), BackendError> {
        // 尝试 WS 断开
        let ws_tx = self.ws_clients.lock().unwrap().remove(client_id);
        if let Some(tx) = ws_tx {
            let _ = tx.send(Message::Close(None)).await;
            return Ok(());
        }
        // 尝试 SSE 断开
        self.sse_clients.lock().unwrap().remove(client_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noop_hooks() -> TransportHooks {
        TransportHooks {
            on_connect: Arc::new(|_| {}),
            on_message: Arc::new(|_, _, _| {}),
            on_disconnect: Arc::new(|_| {}),
        }
    }

    #[test]
    fn to_axum_path_converts_placeholders() {
        assert_eq!(UnifiedServer::to_axum_path("/{event}"), "/:event");
        assert_eq!(UnifiedServer::to_axum_path("/order/{event}"), "/order/:event");
        assert_eq!(UnifiedServer::to_axum_path("/stream"), "/stream");
    }

    #[test]
    fn default_http_routes_correct() {
        let defaults = UnifiedServer::default_http_routes();
        assert_eq!(defaults.len(), 2);
        assert_eq!(defaults[0].method, HttpMethod::Post);
        assert_eq!(defaults[0].path, "/{event}");
        assert_eq!(defaults[1].method, HttpMethod::Get);
        assert_eq!(defaults[1].path, "/stream");
    }

    #[test]
    fn unified_state_ip_filter() {
        let mut sys = SystemSettings::default();
        sys.ip_access.whitelist = vec!["10.0.0.1".to_string()];
        let state = UnifiedState {
            ws_clients: Arc::new(Mutex::new(HashMap::new())),
            sse_clients: Arc::new(Mutex::new(HashMap::new())),
            hooks: noop_hooks(),
            cfg: ServerConfig::default(),
            sys,
            routes: Arc::new(HashMap::new()),
            mock_enabled: false,
            mock_rules: Arc::new(Vec::new()),
            mock_default_status_code: 200,
            mock_default_response_body: "{}".to_string(),
            mock_default_delay_ms: 0,
        };
        assert!(!state.allow_ip("192.168.1.5"));
        assert!(state.allow_ip("10.0.0.1"));
    }

    #[test]
    fn parse_query_basic() {
        let m = parse_query(Some("a=1&b=hello%20world&c"));
        assert_eq!(m.get("a").unwrap().as_str().unwrap(), "1");
        assert_eq!(m.get("b").unwrap().as_str().unwrap(), "hello world");
        assert_eq!(m.get("c").unwrap().as_str().unwrap(), "");
    }

    #[test]
    fn build_router_does_not_panic_with_mock() {
        let cfg = ServerConfig {
            id: "test".to_string(),
            protocol: ProtocolType::Websocket,
            port: 0,
            mock_enabled: true,
            mock_rules: vec![MockRule {
                id: "r1".to_string(),
                name: "test rule".to_string(),
                method: HttpMethod::Get,
                path_pattern: "/users".to_string(),
                response_status_code: 200,
                response_body: "{\"ok\":true}".to_string(),
                enabled: true,
                ..Default::default()
            }],
            mock_default_status_code: 404,
            mock_default_response_body: "{\"error\":\"not found\"}".to_string(),
            ..Default::default()
        };
        let server = Arc::new_cyclic(|weak| {
            UnifiedServer::new(cfg, SystemSettings::default(), noop_hooks(), weak.clone())
        });
        let _router = server.build_router();
    }

    #[test]
    fn build_router_http_protocol_with_routes() {
        let cfg = ServerConfig {
            id: "test".to_string(),
            protocol: ProtocolType::Http,
            port: 0,
            http_routes: vec![
                HttpRouteConfig {
                    id: "r1".to_string(),
                    method: HttpMethod::Post,
                    path: "/{event}".to_string(),
                    route_type: HttpRouteType::Inbound,
                    event: None,
                    description: None,
                },
                HttpRouteConfig {
                    id: "r2".to_string(),
                    method: HttpMethod::Get,
                    path: "/stream".to_string(),
                    route_type: HttpRouteType::Stream,
                    event: None,
                    description: None,
                },
            ],
            mock_enabled: true,
            ..Default::default()
        };
        let server = Arc::new_cyclic(|weak| {
            UnifiedServer::new(cfg, SystemSettings::default(), noop_hooks(), weak.clone())
        });
        let _router = server.build_router();
    }
}

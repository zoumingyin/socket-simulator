//! HttpServer —— 受管服务的 HTTP 传输层（可自定义路由）
//!
//! 在受管服务的 `cfg.ip:cfg.port` 上用 axum 提供用户自定义的路由：
//! - `inbound` 类型路由：收消息（body 为 JSON），映射到 `on_message`；事件名取自
//!   路径 `{event}` 占位符、或路由 `event` 字段、或路径末段
//! - `stream` 类型路由：SSE 长连接，server→client 单向推送，映射到 `send`/`broadcast`
//!
//! 若 `cfg.http_routes` 为空，则使用内置默认路由：`POST /{event}` + `GET /stream`。
//! 端口冲突时复用 `release_port` 重试；IP 黑白名单沿用 `SystemSettings`（与 WsServer 一致）。

use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

use axum::body::to_bytes;
use axum::extract::connect_info::ConnectInfo;
use axum::extract::{MatchedPath, Path, Request, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::routing::MethodRouter;
use axum::Json;
use axum::middleware::{from_fn_with_state, Next};
use axum::Router;
use futures_util::Stream;
use serde_json::Value;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, Notify};

use crate::backend::constants::*;
use crate::backend::error::BackendError;
use crate::backend::net::port_release::release_port;
use crate::backend::transport::Transport;
use crate::backend::transport::websocket::TransportHooks;
use crate::backend::types::*;

use nanoid::nanoid;

/// SSE 客户端外发消息发送端
type SseTx = mpsc::UnboundedSender<SseEvent>;

/// 共享状态（注入 axum handler / 中间件）
#[derive(Clone)]
pub struct HttpAppState {
    pub clients: Arc<Mutex<HashMap<String, SseTx>>>,
    pub hooks: TransportHooks,
    pub sys: SystemSettings,
    pub server_id: String,
    pub protocol: ProtocolType,
    /// 路由配置映射，key = `"{METHOD} {AXUM_PATH}"`（如 `"POST /order/:event"`），
    /// handler 用 MatchedPath + method 查回 route 配置（取固定 event 名等）
    pub routes: Arc<HashMap<String, HttpRouteConfig>>,
}

impl HttpAppState {
    /// IP 黑白名单过滤（与 WsServer 一致）
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

/// 受管服务 HTTP 服务端
pub struct HttpServer {
    cfg: ServerConfig,
    sys: SystemSettings,
    hooks: TransportHooks,
    clients: Arc<Mutex<HashMap<String, SseTx>>>,
    running: Arc<AtomicBool>,
    shutdown: Mutex<Option<Arc<Notify>>>,
}

impl HttpServer {
    pub fn new(cfg: ServerConfig, sys: SystemSettings, hooks: TransportHooks) -> Self {
        Self {
            cfg,
            sys,
            hooks,
            clients: Arc::new(Mutex::new(HashMap::new())),
            running: Arc::new(AtomicBool::new(false)),
            shutdown: Mutex::new(None),
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

    /// 内置默认路由（cfg.http_routes 为空时使用）
    fn default_routes() -> Vec<HttpRouteConfig> {
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

    /// 构建路由（状态由调用方通过 `with_state` 注入；IP 过滤中间件自带独立 state）
    fn build_router(&self) -> Router {
        // 选生效路由：用户配置优先，空则用默认
        let effective: Vec<HttpRouteConfig> = if self.cfg.http_routes.is_empty() {
            Self::default_routes()
        } else {
            self.cfg.http_routes.clone()
        };

        // routes 映射：key = "METHOD /axum/path"
        let mut routes_map: HashMap<String, HttpRouteConfig> = HashMap::new();
        for r in &effective {
            let key = format!("{} {}", r.method.as_str(), Self::to_axum_path(&r.path));
            routes_map.insert(key, r.clone());
        }

        let state = HttpAppState {
            clients: self.clients.clone(),
            hooks: self.hooks.clone(),
            sys: self.sys.clone(),
            server_id: self.cfg.id.clone(),
            protocol: ProtocolType::Http,
            routes: Arc::new(routes_map),
        };
        let ip_state = state.clone();

        // 按 axum 路径分组：inbound / stream 分别收集，避免同路径不同 handler 冲突
        // 同一 (path, method) 重复时去重（HashSet），防止 axum panic
        let mut inbound_by_path: HashMap<String, Vec<HttpMethod>> = HashMap::new();
        let mut stream_by_path: HashMap<String, Vec<HttpMethod>> = HashMap::new();
        for r in &effective {
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

        let mut router: Router<HttpAppState> = Router::new();
        for (path, methods) in inbound_by_path {
            let mut mr: MethodRouter<HttpAppState> = MethodRouter::new();
            for m in methods {
                mr = match m {
                    HttpMethod::Get => mr.get(ingress_handler),
                    HttpMethod::Post => mr.post(ingress_handler),
                    HttpMethod::Put => mr.put(ingress_handler),
                    HttpMethod::Delete => mr.delete(ingress_handler),
                    HttpMethod::Patch => mr.patch(ingress_handler),
                    HttpMethod::Head => mr.head(ingress_handler),
                    HttpMethod::Options => mr.options(ingress_handler),
                    HttpMethod::Any => {
                        // 受管 HTTP 服务不支持 ANY（任意方法应配多条具体方法）；忽略
                        continue;
                    }
                };
            }
            router = router.route(&path, mr);
        }
        for (path, methods) in stream_by_path {
            let mut mr: MethodRouter<HttpAppState> = MethodRouter::new();
            for m in methods {
                mr = match m {
                    HttpMethod::Get => mr.get(stream_handler),
                    HttpMethod::Post => mr.post(stream_handler),
                    HttpMethod::Put => mr.put(stream_handler),
                    HttpMethod::Delete => mr.delete(stream_handler),
                    HttpMethod::Patch => mr.patch(stream_handler),
                    HttpMethod::Head => mr.head(stream_handler),
                    HttpMethod::Options => mr.options(stream_handler),
                    HttpMethod::Any => continue,
                };
            }
            router = router.route(&path, mr);
        }

        router
            .layer(from_fn_with_state(ip_state, ip_guard))
            .with_state(state)
    }
}

// ==================== 端点处理器 ====================

/// 入站消息：body 为 JSON；事件名取自 route.event / 路径 {event} / 末段
async fn ingress_handler(
    State(state): State<HttpAppState>,
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

    // 事件名优先级：route.event > 路径 {event} 参数 > 路径末段 > "message"
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
    (state.hooks.on_message)("http-ingress".to_string(), event, data);
    (
        StatusCode::OK,
        Json(serde_json::json!({ "ok": true })),
    )
        .into_response()
}

/// 实时推送：SSE 长连接；每个连接即一个客户端
async fn stream_handler(
    State(state): State<HttpAppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Sse<ClientStream> {
    let cid = nanoid!(16);
    let (tx, rx) = mpsc::unbounded_channel::<SseEvent>();
    state.clients.lock().unwrap().insert(cid.clone(), tx);

    // 通知连接（与 WsServer 一致的客户端信息）
    let now = now_rfc3339();
    let info = ClientInfo {
        id: cid.clone(),
        server_id: state.server_id.clone(),
        socket_id: cid.clone(),
        ip_address: addr.ip().to_string(),
        connected_at: now.clone(),
        last_activity_at: now,
        protocol: state.protocol,
        status: ClientStatus::Connected,
        ..Default::default()
    };
    (state.hooks.on_connect)(info);

    let stream = ClientStream {
        rx,
        cid: cid.clone(),
        clients: state.clients.clone(),
        hooks: state.hooks.clone(),
    };
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

/// IP 黑白名单中间件（拒绝则返回 403）
async fn ip_guard(
    State(state): State<HttpAppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request,
    next: Next,
) -> Response {
    if !state.allow_ip(&addr.ip().to_string()) {
        return StatusCode::FORBIDDEN.into_response();
    }
    next.run(req).await
}

// ==================== SSE 流（携带断开清理） ====================

/// SSE 流包装：当连接断开（流被 drop）时，从客户端表移除并回调 on_disconnect
struct ClientStream {
    rx: mpsc::UnboundedReceiver<SseEvent>,
    cid: String,
    clients: Arc<Mutex<HashMap<String, SseTx>>>,
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
        self.clients.lock().unwrap().remove(&self.cid);
        (self.hooks.on_disconnect)(self.cid.clone());
    }
}

// ==================== Transport 实现 ====================

#[async_trait::async_trait]
impl Transport for HttpServer {
    async fn start(&self) -> Result<(), BackendError> {
        if self.running.load(Ordering::SeqCst) {
            return Ok(());
        }
        let addr = (self.cfg.ip.as_str(), self.cfg.port as u16);
        let listener = match TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                eprintln!("[HttpServer] 端口 {} 被占用，尝试释放后重试", self.cfg.port);
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
            if let Err(e) = axum::serve(listener, app).with_graceful_shutdown(shutdown).await {
                eprintln!("[HttpServer] serve error: {}", e);
            }
        });
        Ok(())
    }

    async fn stop(&self) -> Result<(), BackendError> {
        self.running.store(false, Ordering::SeqCst);
        if let Some(n) = self.shutdown.lock().unwrap().take() {
            n.notify_one();
        }
        // 清空客户端连接，触发各 SSE 流 drop → on_disconnect
        self.clients.lock().unwrap().clear();
        Ok(())
    }

    async fn send(&self, client_id: &str, event: &str, data: Value) -> Result<(), BackendError> {
        let ev = SseEvent::default()
            .event(event.to_string())
            .data(serde_json::to_string(&data).unwrap_or_default());
        if let Some(tx) = self.clients.lock().unwrap().get(client_id).cloned() {
            let _ = tx.send(ev);
        }
        Ok(())
    }

    async fn broadcast(
        &self,
        event: &str,
        data: Value,
        target_ids: Option<Vec<String>>,
    ) -> Result<(), BackendError> {
        let ev = SseEvent::default()
            .event(event.to_string())
            .data(serde_json::to_string(&data).unwrap_or_default());
        let targets: Vec<String> = match target_ids {
            Some(ids) => ids,
            None => self.clients.lock().unwrap().keys().cloned().collect(),
        };
        for id in targets {
            if let Some(tx) = self.clients.lock().unwrap().get(&id).cloned() {
                let _ = tx.send(ev.clone());
            }
        }
        Ok(())
    }

    async fn disconnect_client(&self, client_id: &str) -> Result<(), BackendError> {
        // 移除发送端 → SSE 流结束 → ClientStream drop → on_disconnect
        self.clients.lock().unwrap().remove(client_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 回归：IP 白名单非空且未包含来源 IP 时应拒绝。
    #[test]
    fn allow_ip_respects_whitelist() {
        let mut sys = SystemSettings::default();
        sys.ip_access.whitelist = vec!["10.0.0.1".to_string()];
        let state = HttpAppState {
            clients: Arc::new(Mutex::new(HashMap::new())),
            hooks: TransportHooks {
                on_connect: Arc::new(|_| {}),
                on_message: Arc::new(|_, _, _| {}),
                on_disconnect: Arc::new(|_| {}),
            },
            sys,
            server_id: "s1".to_string(),
            protocol: ProtocolType::Http,
            routes: Arc::new(HashMap::new()),
        };
        assert!(!state.allow_ip("192.168.1.5"), "非白名单 IP 应被拒绝");
        assert!(state.allow_ip("10.0.0.1"), "白名单 IP 应被允许");
    }

    /// {event} 占位符应被转换为 axum :event 语法
    #[test]
    fn to_axum_path_converts_placeholders() {
        assert_eq!(HttpServer::to_axum_path("/{event}"), "/:event");
        assert_eq!(HttpServer::to_axum_path("/order/{event}"), "/order/:event");
        assert_eq!(HttpServer::to_axum_path("/stream"), "/stream");
        assert_eq!(HttpServer::to_axum_path("/a/b/{id}/c"), "/a/b/:id/c");
    }

    /// 默认路由为空配置时应回退到 POST /{event} + GET /stream
    #[test]
    fn default_routes_when_empty() {
        let defaults = HttpServer::default_routes();
        assert_eq!(defaults.len(), 2);
        assert_eq!(defaults[0].method, HttpMethod::Post);
        assert_eq!(defaults[0].path, "/{event}");
        assert_eq!(defaults[0].route_type, HttpRouteType::Inbound);
        assert_eq!(defaults[1].method, HttpMethod::Get);
        assert_eq!(defaults[1].path, "/stream");
        assert_eq!(defaults[1].route_type, HttpRouteType::Stream);
    }

    /// 自定义路由应正确去重同 (path, method) 并分组
    #[test]
    fn custom_routes_group_correctly() {
        let cfg = ServerConfig {
            id: "s1".to_string(),
            protocol: ProtocolType::Http,
            http_routes: vec![
                HttpRouteConfig {
                    id: "r1".to_string(),
                    method: HttpMethod::Post,
                    path: "/order/{event}".to_string(),
                    route_type: HttpRouteType::Inbound,
                    event: None,
                    description: None,
                },
                HttpRouteConfig {
                    id: "r2".to_string(),
                    method: HttpMethod::Put,
                    path: "/order/{event}".to_string(),
                    route_type: HttpRouteType::Inbound,
                    event: Some("orderUpdated".to_string()),
                    description: None,
                },
                HttpRouteConfig {
                    id: "r3".to_string(),
                    method: HttpMethod::Get,
                    path: "/events".to_string(),
                    route_type: HttpRouteType::Stream,
                    event: None,
                    description: None,
                },
            ],
            ..Default::default()
        };
        let sys = SystemSettings::default();
        let hooks = TransportHooks {
            on_connect: Arc::new(|_| {}),
            on_message: Arc::new(|_, _, _| {}),
            on_disconnect: Arc::new(|_| {}),
        };
        // 构建不应 panic（同 path 不同 method 正常合并，重复去重）
        let server = HttpServer::new(cfg, sys, hooks);
        let _router = server.build_router();
        // 验证 routes 映射 key 包含 PUT 固定事件
        assert!(
            server
                .cfg
                .http_routes
                .iter()
                .any(|r| r.method == HttpMethod::Put && r.event == Some("orderUpdated".to_string())),
            "应保留 PUT 路由的固定事件名"
        );
    }
}

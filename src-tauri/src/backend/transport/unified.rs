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
//! 1. 显式注册的 HTTP transport 路由（inbound/stream，仅 protocol=http 时，由共享 `http_routing` 构建）
//! 2. Fallback 统一处理器（WS 升级检测 → Mock dispatch）
//! 3. IP 黑白名单中间件（最外层，覆盖全部路由与 fallback）
//! 4. CORS（最外层）
//!
//! ## 互不干扰保证
//! - WebSocket 连接的 error/timeout/disconnect 逻辑完全独立（与 WsServer 一致）
//! - Mock HTTP 的 error/delay 逻辑完全独立（与 MockServer dispatch 一致）
//! - 两者在同一个 Router 的不同分支中执行，不共享可变状态
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
//!
//! ## 复用说明
//! HTTP transport 的路由构建与 ingress/stream/SSE 处理器、查询串解析、IP 中间件
//! 均收敛于 `transport::http_routing`（与 `HttpServer` 共用），本模块仅实现
//! `HttpRouteState` 并保留 axum-WS 连接循环（与 `WsServer` 的 `tokio_tungstenite`
//! 属于不同 WS 库，无法共享）。

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use axum::body::to_bytes;
use axum::extract::connect_info::ConnectInfo;
use axum::extract::ws::{WebSocket, WebSocketUpgrade};
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::from_fn_with_state;
use axum::response::sse::Event as SseEvent;
use axum::response::Response;
use axum::Router;
use serde_json::Value;
use tokio::sync::{mpsc, Notify};
use futures_util::StreamExt; // 提供 `WebSocket::split()`（handle_ws_connection 拆流用）

use crate::backend::error::BackendError;
use crate::backend::mock::engine::{MockEndpoint, MockRequest};
use crate::backend::mock::responder::json_error_response;
use crate::backend::net::bind::bind_with_release;
use crate::backend::transport::http_routing::{
    self, build_http_router, default_http_routes, HttpRouteState, ip_guard, parse_query, SseTx,
};
use crate::backend::transport::ws_connection::{
    frame_to_text, pump_ws, AxumAdapter, WireMsg, WsClientRegistry,
};
use crate::backend::transport::Transport;
use crate::backend::transport::hooks::TransportHooks;
use crate::backend::types::*;

use nanoid::nanoid;
use tower_http::cors::CorsLayer;

const MAX_BODY_SIZE: usize = 16 * 1024 * 1024; // 16MB

/// 统一路由共享状态
///
/// 同时承载 Socket 传输层和 Mock HTTP 引擎所需的状态。
/// 两者在 fallback handler 中通过请求类型自动分发，互不共享可变状态。
///
/// `HttpRouteState` 的实现把 HTTP transport 所需的 hooks/routes/sse 等字段暴露给
/// 共享的 `http_routing` 处理器；IP 过滤与 Mock 分发逻辑仍在本模块内。
#[derive(Clone)]
pub struct UnifiedState {
    // ---------- Socket 传输层状态 ----------
    /// WS 客户端表：raw socketId → 外发消息发送端（统一为 WireMsg）
    pub ws_clients: WsClientRegistry,
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

impl HttpRouteState for UnifiedState {
    fn hooks(&self) -> &TransportHooks {
        &self.hooks
    }
    fn routes(&self) -> &Arc<HashMap<String, HttpRouteConfig>> {
        &self.routes
    }
    fn server_id(&self) -> &str {
        &self.cfg.id
    }
    fn protocol(&self) -> ProtocolType {
        self.cfg.protocol
    }
    fn sse_clients(&self) -> &Arc<Mutex<HashMap<String, SseTx>>> {
        &self.sse_clients
    }
    fn allow_ip(&self, ip: &str) -> bool {
        crate::backend::net::ip_access::allow_ip(
            &self.sys.ip_access.whitelist,
            &self.sys.ip_access.blacklist,
            ip,
        )
    }
    fn ip_denied(&self, ip: &str) -> Response {
        json_error_response(
            StatusCode::FORBIDDEN,
            "Forbidden",
            &format!("IP address {} is not allowed", ip),
        )
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
/// 1. 显式 HTTP transport 路由优先匹配（共享 `http_routing` 构建）
/// 2. 未匹配的请求进入 fallback：检测 WS 升级头 → WS 或 Mock
pub struct UnifiedServer {
    cfg: ServerConfig,
    sys: SystemSettings,
    hooks: TransportHooks,
    ws_clients: WsClientRegistry,
    sse_clients: Arc<Mutex<HashMap<String, SseTx>>>,
    running: Arc<AtomicBool>,
    shutdown: Mutex<Option<Arc<Notify>>>,
}

impl UnifiedServer {
    /// 构造
    pub fn new(cfg: ServerConfig, sys: SystemSettings, hooks: TransportHooks) -> Self {
        Self {
            cfg,
            sys,
            hooks,
            ws_clients: Arc::new(Mutex::new(HashMap::new())),
            sse_clients: Arc::new(Mutex::new(HashMap::new())),
            running: Arc::new(AtomicBool::new(false)),
            shutdown: Mutex::new(None),
        }
    }

    /// 构建统一路由
    ///
    /// 路由结构（按优先级从高到低）：
    /// 1. HTTP transport 显式路由（inbound/stream，仅 protocol=http 时注册，由共享 `http_routing` 构建）
    /// 2. Fallback → `unified_fallback`（检测 WS 升级 → WS handler；否则 → Mock dispatch）
    /// 3. IP 黑白名单中间件（最外层，覆盖全部路由与 fallback）
    /// 4. CORS（最外层）
    fn build_router(&self) -> Router {
        // 选生效 HTTP transport 路由：仅 http 协议时注册
        let effective_http_routes: Vec<HttpRouteConfig> =
            if self.cfg.protocol == ProtocolType::Http {
                if self.cfg.http_routes.is_empty() {
                    default_http_routes()
                } else {
                    self.cfg.http_routes.clone()
                }
            } else {
                Vec::new()
            };

        let state = UnifiedState {
            ws_clients: self.ws_clients.clone(),
            sse_clients: self.sse_clients.clone(),
            hooks: self.hooks.clone(),
            cfg: self.cfg.clone(),
            sys: self.sys.clone(),
            routes: Arc::new(http_routing::build_routes_map(&effective_http_routes)),
            mock_enabled: self.cfg.mock_enabled,
            mock_rules: Arc::new(self.cfg.mock_rules.clone()),
            mock_default_status_code: self.cfg.mock_default_status_code,
            mock_default_response_body: self.cfg.mock_default_response_body.clone(),
            mock_default_delay_ms: self.cfg.mock_default_delay_ms,
        };

        build_http_router(effective_http_routes, state.clone())
            .fallback(unified_fallback)
            .layer(CorsLayer::permissive())
            .layer(from_fn_with_state(state.clone(), ip_guard::<UnifiedState>))
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
async fn handle_ws_connection(state: UnifiedState, socket: WebSocket, raw_id: String, ip: String) {
    let (write, read) = socket.split();
    let (out_tx, out_rx) = mpsc::channel::<WireMsg>(64);
    state.ws_clients.lock().unwrap().insert(raw_id.clone(), out_tx);

    pump_ws::<AxumAdapter, _, _>(
        read,
        write,
        raw_id.clone(),
        ip,
        state.cfg.clone(),
        state.hooks.clone(),
        out_rx,
    )
    .await;

    // 清理
    state.ws_clients.lock().unwrap().remove(&raw_id);
    (state.hooks.on_disconnect)(raw_id);
}

// ==================== Mock HTTP 引擎 ====================

/// Mock HTTP 分发 —— 纯函数，处理普通 HTTP 请求
///
/// 独立的 error/delay 逻辑：
/// - 规则命中延迟 → tokio::sleep
/// - 默认延迟 → tokio::sleep
/// - 未启用 → 404 JSON 错误（由 `unified_fallback` 处理）
/// - IP 禁止 → 由最外层 `ip_guard` 中间件拦截
///
/// 与 WS 连接处理完全隔离，不共享可变状态。
///
/// 注：规则匹配与 `MockServer::dispatch` 一致，但保留 `rule.enabled` 过滤
/// （unified 模式下用户可在 UI 临时禁用某条规则，而独立 Mock 服务不暴露该开关）。
/// 查询串解析复用 `http_routing::parse_query`。
async fn mock_dispatch(state: &UnifiedState, req: Request) -> Response {
    // 在 into_parts 前拷出只读数据
    let method = req.method().as_str().to_string();
    let full_path = req.uri().path().to_string();
    let query_q = req.uri().query().map(|s| s.to_string());
    let query_map = parse_query(query_q.as_deref());

    let (parts, body) = req.into_parts();
    let body_bytes = to_bytes(body, MAX_BODY_SIZE).await.unwrap_or_default();

    // 共端口：规则匹配用的 path 即请求完整路径（无 base_path 概念）；
    // 规则级 enabled 过滤由 MockEngine 内部的 match_rule 统一处理。
    let endpoint = MockEndpoint {
        rules: state.mock_rules.as_ref().clone(),
        default_status_code: state.mock_default_status_code,
        default_response_body: state.mock_default_response_body.clone(),
        default_delay_ms: state.mock_default_delay_ms,
    };
    let req_meta = MockRequest {
        method: &method,
        path: &full_path,
        headers: &parts.headers,
        query: &query_map,
        body: &body_bytes,
    };
    crate::backend::mock::engine::dispatch(&endpoint, &req_meta).await
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

        let listener = bind_with_release(self.cfg.ip.as_str(), self.cfg.port as u16).await?;

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
        let ws_msg = WireMsg::Text(frame_to_text(event, &data));
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
        let ws_payload = WireMsg::Text(frame_to_text(event, &data.clone()));
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
            let _ = tx.send(WireMsg::Close).await;
            return Ok(());
        }
        // 尝试 SSE 断开
        self.sse_clients.lock().unwrap().remove(client_id);
        Ok(())
    }
}

impl crate::backend::transport::ProtocolAdapter for UnifiedServer {
    fn protocol(&self) -> ProtocolType {
        self.cfg.protocol
    }

    fn server_id(&self) -> &str {
        &self.cfg.id
    }

    fn is_unified(&self) -> bool {
        true
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
        assert_eq!(http_routing::to_axum_path("/{event}"), "/:event");
        assert_eq!(
            http_routing::to_axum_path("/order/{event}"),
            "/order/:event"
        );
        assert_eq!(http_routing::to_axum_path("/stream"), "/stream");
    }

    #[test]
    fn default_http_routes_correct() {
        let defaults = default_http_routes();
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
        let server = Arc::new(UnifiedServer::new(cfg, SystemSettings::default(), noop_hooks()));
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
        let server = Arc::new(UnifiedServer::new(cfg, SystemSettings::default(), noop_hooks()));
        let _router = server.build_router();
    }
}

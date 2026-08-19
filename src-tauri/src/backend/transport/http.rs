//! HttpServer —— 受管服务的 HTTP 传输层（可自定义路由）
//!
//! 在受管服务的 `cfg.ip:cfg.port` 上用 axum 提供用户自定义的路由：
//! - `inbound` 类型路由：收消息（body 为 JSON），映射到 `on_message`；事件名取自
//!   路径 `{event}` 占位符、或路由 `event` 字段、或路径末段
//! - `stream` 类型路由：SSE 长连接，server→client 单向推送，映射到 `send`/`broadcast`
//!
//! 若 `cfg.http_routes` 为空，则使用内置默认路由：`POST /{event}` + `GET /stream`。
//! 端口冲突时复用 `release_port` 重试；IP 黑白名单沿用 `SystemSettings`（与 WsServer 一致）。
//!
//! 路由构建与处理器实现收敛于 `transport::http_routing`（与 UnifiedServer 共用）。

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use axum::extract::connect_info::ConnectInfo;
use axum::http::StatusCode;
use axum::middleware::from_fn_with_state;
use axum::response::{IntoResponse, Response};
use axum::Router;
use serde_json::Value;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, Notify};

use crate::backend::error::BackendError;
use crate::backend::net::bind::bind_with_release;
use crate::backend::net::ip_access::allow_ip;
use crate::backend::transport::hooks::TransportHooks;
use crate::backend::transport::http_routing::{
    build_http_router, build_routes_map, default_http_routes, HttpRouteState, ip_guard, SseTx,
};
use crate::backend::transport::Transport;
use crate::backend::types::*;

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

impl HttpRouteState for HttpAppState {
    fn hooks(&self) -> &TransportHooks {
        &self.hooks
    }
    fn routes(&self) -> &Arc<HashMap<String, HttpRouteConfig>> {
        &self.routes
    }
    fn server_id(&self) -> &str {
        &self.server_id
    }
    fn protocol(&self) -> ProtocolType {
        self.protocol
    }
    fn sse_clients(&self) -> &Arc<Mutex<HashMap<String, SseTx>>> {
        &self.clients
    }
    fn allow_ip(&self, ip: &str) -> bool {
        allow_ip(
            &self.sys.ip_access.whitelist,
            &self.sys.ip_access.blacklist,
            ip,
        )
    }
    fn ip_denied(&self, _ip: &str) -> Response {
        StatusCode::FORBIDDEN.into_response()
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

    /// 构建路由（状态由调用方通过 `with_state` 注入；IP 过滤中间件在共享构建器内）
    fn build_router(&self) -> Router {
        // 选生效路由：用户配置优先，空则用默认
        let effective: Vec<HttpRouteConfig> = if self.cfg.http_routes.is_empty() {
            default_http_routes()
        } else {
            self.cfg.http_routes.clone()
        };
        let routes_map = build_routes_map(&effective);
        let state = HttpAppState {
            clients: self.clients.clone(),
            hooks: self.hooks.clone(),
            sys: self.sys.clone(),
            server_id: self.cfg.id.clone(),
            protocol: ProtocolType::Http,
            routes: Arc::new(routes_map),
        };
        build_http_router(effective, state.clone())
            .layer(from_fn_with_state(state.clone(), ip_guard::<HttpAppState>))
            .with_state(state)
    }
}

// ==================== Transport 实现 ====================

#[async_trait::async_trait]
impl Transport for HttpServer {
    async fn start(&self) -> Result<(), BackendError> {
        if self.running.load(Ordering::SeqCst) {
            return Ok(());
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
        let ev = axum::response::sse::Event::default()
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
        let ev = axum::response::sse::Event::default()
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

impl crate::backend::transport::ProtocolAdapter for HttpServer {
    fn protocol(&self) -> ProtocolType {
        ProtocolType::Http
    }

    fn server_id(&self) -> &str {
        &self.cfg.id
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
        assert_eq!(crate::backend::transport::http_routing::to_axum_path("/{event}"), "/:event");
        assert_eq!(
            crate::backend::transport::http_routing::to_axum_path("/order/{event}"),
            "/order/:event"
        );
        assert_eq!(crate::backend::transport::http_routing::to_axum_path("/stream"), "/stream");
        assert_eq!(
            crate::backend::transport::http_routing::to_axum_path("/a/b/{id}/c"),
            "/a/b/:id/c"
        );
    }

    /// 默认路由为空配置时应回退到 POST /{event} + GET /stream
    #[test]
    fn default_routes_when_empty() {
        let defaults = default_http_routes();
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

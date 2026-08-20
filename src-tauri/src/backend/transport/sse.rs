//! SSE 传输层骨架（P2-4 预留）
//!
//! 真实实现需：提供 HTTP 端点，客户端以 `text/event-stream` 长连接订阅；服务端单向
//! 推送 `event: <name>\ndata: <json>\n\n`。入站（如有）映射为 `WsFrame` 调
//! `self.hooks.on_message`；连接/断开调 `on_connect`/`on_disconnect`。SSE 通常配合
//! HTTP 路由（参照 `HttpServer` / `unified`），按 `self.sys` 做 IP 过滤。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde_json::Value;

use crate::backend::error::BackendError;
use crate::backend::transport::hooks::TransportHooks;
use crate::backend::transport::{ProtocolAdapter, Transport};
use crate::backend::types::{ProtocolType, ServerConfig, SystemSettings};

/// 受管服务 SSE 传输层骨架（尚未实现）
pub struct SseAdapter {
    cfg: ServerConfig,
    sys: SystemSettings,
    hooks: TransportHooks,
    running: Arc<AtomicBool>,
}

impl SseAdapter {
    pub fn new(cfg: ServerConfig, sys: SystemSettings, hooks: TransportHooks) -> Arc<Self> {
        Arc::new(Self {
            cfg,
            sys,
            hooks,
            running: Arc::new(AtomicBool::new(false)),
        })
    }
}

#[async_trait::async_trait]
impl Transport for SseAdapter {
    async fn start(&self) -> Result<(), BackendError> {
        Err(BackendError::not_implemented(
            "sse adapter is a reserved skeleton, not yet implemented",
        ))
    }

    async fn stop(&self) -> Result<(), BackendError> {
        self.running.store(false, Ordering::SeqCst);
        Ok(())
    }

    async fn send(&self, _client_id: &str, _event: &str, _data: Value) -> Result<(), BackendError> {
        Err(BackendError::not_implemented(
            "sse adapter is a reserved skeleton, not yet implemented",
        ))
    }

    async fn broadcast(
        &self,
        _event: &str,
        _data: Value,
        _target_ids: Option<Vec<String>>,
    ) -> Result<(), BackendError> {
        Err(BackendError::not_implemented(
            "sse adapter is a reserved skeleton, not yet implemented",
        ))
    }

    async fn disconnect_client(&self, _client_id: &str) -> Result<(), BackendError> {
        Err(BackendError::not_implemented(
            "sse adapter is a reserved skeleton, not yet implemented",
        ))
    }
}

#[async_trait::async_trait]
impl ProtocolAdapter for SseAdapter {
    fn protocol(&self) -> ProtocolType {
        ProtocolType::Sse
    }

    fn server_id(&self) -> &str {
        &self.cfg.id
    }

    fn is_unified(&self) -> bool {
        false
    }
}

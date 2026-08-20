//! TCP 传输层骨架（P2-4 预留）
//!
//! 真实实现需：绑定 `TcpListener` + accept loop，为每个连接分配 `client_id`，
//! 按协议自有分帧规则解析字节流为 `WsFrame{event,data}`，在连接/消息/断开时调用
//! `self.hooks` 的 `on_connect`/`on_message`/`on_disconnect`，并应用 `self.sys` 的
//! IP 黑白名单与最大连接数限制（参照 `WsServer`）。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde_json::Value;

use crate::backend::error::BackendError;
use crate::backend::transport::hooks::TransportHooks;
use crate::backend::transport::{ProtocolAdapter, Transport};
use crate::backend::types::{ProtocolType, ServerConfig, SystemSettings};

/// 受管服务 TCP 传输层骨架（尚未实现）
pub struct TcpAdapter {
    cfg: ServerConfig,
    sys: SystemSettings,
    hooks: TransportHooks,
    running: Arc<AtomicBool>,
}

impl TcpAdapter {
    /// 构造（骨架无需 self 弱引用；真实实现如需 accept loop 取 Arc<Self> 可改为 `Arc::new_cyclic`）
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
impl Transport for TcpAdapter {
    async fn start(&self) -> Result<(), BackendError> {
        Err(BackendError::not_implemented(
            "tcp adapter is a reserved skeleton, not yet implemented",
        ))
    }

    async fn stop(&self) -> Result<(), BackendError> {
        self.running.store(false, Ordering::SeqCst);
        Ok(())
    }

    async fn send(&self, _client_id: &str, _event: &str, _data: Value) -> Result<(), BackendError> {
        Err(BackendError::not_implemented(
            "tcp adapter is a reserved skeleton, not yet implemented",
        ))
    }

    async fn broadcast(
        &self,
        _event: &str,
        _data: Value,
        _target_ids: Option<Vec<String>>,
    ) -> Result<(), BackendError> {
        Err(BackendError::not_implemented(
            "tcp adapter is a reserved skeleton, not yet implemented",
        ))
    }

    async fn disconnect_client(&self, _client_id: &str) -> Result<(), BackendError> {
        Err(BackendError::not_implemented(
            "tcp adapter is a reserved skeleton, not yet implemented",
        ))
    }
}

#[async_trait::async_trait]
impl ProtocolAdapter for TcpAdapter {
    fn protocol(&self) -> ProtocolType {
        ProtocolType::Tcp
    }

    fn server_id(&self) -> &str {
        &self.cfg.id
    }

    fn is_unified(&self) -> bool {
        false
    }
}

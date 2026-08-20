//! UDP 传输层骨架（P2-4 预留）
//!
//! 真实实现需：绑定 `UdpSocket`（无连接、数据报），按 `client_addr` 维护客户端映射，
//! 解析数据报为 `WsFrame{event,data}`，在收到消息时调用 `self.hooks.on_message`
//! （UDP 无显式 connect/disconnect，可用超时回收客户端）。同样应用 `self.sys` 的
//! IP 黑白名单与最大连接数限制。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde_json::Value;

use crate::backend::error::BackendError;
use crate::backend::transport::hooks::TransportHooks;
use crate::backend::transport::{ProtocolAdapter, Transport};
use crate::backend::types::{ProtocolType, ServerConfig, SystemSettings};

/// 受管服务 UDP 传输层骨架（尚未实现）
pub struct UdpAdapter {
    /// 预留字段：真实实现（数据报收发/客户端注册表/hooks 调用）时使用
    #[allow(dead_code)]
    cfg: ServerConfig,
    #[allow(dead_code)]
    sys: SystemSettings,
    #[allow(dead_code)]
    hooks: TransportHooks,
    running: Arc<AtomicBool>,
}

impl UdpAdapter {
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
impl Transport for UdpAdapter {
    async fn start(&self) -> Result<(), BackendError> {
        Err(BackendError::not_implemented(
            "udp adapter is a reserved skeleton, not yet implemented",
        ))
    }

    async fn stop(&self) -> Result<(), BackendError> {
        self.running.store(false, Ordering::SeqCst);
        Ok(())
    }

    async fn send(&self, _client_id: &str, _event: &str, _data: Value) -> Result<(), BackendError> {
        Err(BackendError::not_implemented(
            "udp adapter is a reserved skeleton, not yet implemented",
        ))
    }

    async fn broadcast(
        &self,
        _event: &str,
        _data: Value,
        _target_ids: Option<Vec<String>>,
    ) -> Result<(), BackendError> {
        Err(BackendError::not_implemented(
            "udp adapter is a reserved skeleton, not yet implemented",
        ))
    }

    async fn disconnect_client(&self, _client_id: &str) -> Result<(), BackendError> {
        Err(BackendError::not_implemented(
            "udp adapter is a reserved skeleton, not yet implemented",
        ))
    }
}

#[async_trait::async_trait]
impl ProtocolAdapter for UdpAdapter {
    fn protocol(&self) -> ProtocolType {
        ProtocolType::Udp
    }

    fn server_id(&self) -> &str {
        &self.cfg.id
    }

    fn is_unified(&self) -> bool {
        false
    }
}

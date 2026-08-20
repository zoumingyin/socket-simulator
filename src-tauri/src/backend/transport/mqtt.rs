//! MQTT 传输层骨架（P2-4 预留）
//!
//! 真实实现需：连接外部 MQTT broker（如 `mqtt://host:port`），按 `ServerConfig` 的
//! topic 配置订阅/发布；入站消息映射为 `WsFrame{event,data}` 后调用
//! `self.hooks.on_message`，连接/断开调用 `on_connect`/`on_disconnect`。broker 地址、
//! client id、用户名密码、TLS 等应来自 `self.cfg` / `self.sys`。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde_json::Value;

use crate::backend::error::BackendError;
use crate::backend::transport::hooks::TransportHooks;
use crate::backend::transport::{ProtocolAdapter, Transport};
use crate::backend::types::{ProtocolType, ServerConfig, SystemSettings};

/// 受管服务 MQTT 传输层骨架（尚未实现）
pub struct MqttAdapter {
    cfg: ServerConfig,
    sys: SystemSettings,
    hooks: TransportHooks,
    running: Arc<AtomicBool>,
}

impl MqttAdapter {
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
impl Transport for MqttAdapter {
    async fn start(&self) -> Result<(), BackendError> {
        Err(BackendError::not_implemented(
            "mqtt adapter is a reserved skeleton, not yet implemented",
        ))
    }

    async fn stop(&self) -> Result<(), BackendError> {
        self.running.store(false, Ordering::SeqCst);
        Ok(())
    }

    async fn send(&self, _client_id: &str, _event: &str, _data: Value) -> Result<(), BackendError> {
        Err(BackendError::not_implemented(
            "mqtt adapter is a reserved skeleton, not yet implemented",
        ))
    }

    async fn broadcast(
        &self,
        _event: &str,
        _data: Value,
        _target_ids: Option<Vec<String>>,
    ) -> Result<(), BackendError> {
        Err(BackendError::not_implemented(
            "mqtt adapter is a reserved skeleton, not yet implemented",
        ))
    }

    async fn disconnect_client(&self, _client_id: &str) -> Result<(), BackendError> {
        Err(BackendError::not_implemented(
            "mqtt adapter is a reserved skeleton, not yet implemented",
        ))
    }
}

#[async_trait::async_trait]
impl ProtocolAdapter for MqttAdapter {
    fn protocol(&self) -> ProtocolType {
        ProtocolType::Mqtt
    }

    fn server_id(&self) -> &str {
        &self.cfg.id
    }

    fn is_unified(&self) -> bool {
        false
    }
}

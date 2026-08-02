//! Transport trait（≡ ITransport）
//!
//! 受管 Socket 服务的传输层抽象。本期仅 `websocket` 协议真正实现；`socket.io` 协议入口
//! 字段保留但标记不支持/降级（见 PRD 待确认问题 e）。

use async_trait::async_trait;
use serde_json::Value;

use crate::backend::error::BackendError;
use crate::backend::types::ProtocolType;

#[async_trait]
pub trait Transport: Send + Sync {
    /// 协议类型
    fn protocol(&self) -> ProtocolType;

    /// 启动传输层（绑定监听并开始 accept loop）
    async fn start(&self) -> Result<(), BackendError>;

    /// 停止传输层
    async fn stop(&self) -> Result<(), BackendError>;

    /// 向指定客户端发送消息
    async fn send(&self, client_id: &str, event: &str, data: Value) -> Result<(), BackendError>;

    /// 广播消息（target_ids 为空表示广播给全部在线客户端）
    async fn broadcast(
        &self,
        event: &str,
        data: Value,
        target_ids: Option<Vec<String>>,
    ) -> Result<(), BackendError>;

    /// 断开指定客户端
    async fn disconnect_client(&self, client_id: &str) -> Result<(), BackendError>;

    /// 是否正在运行
    fn is_running(&self) -> bool;
}

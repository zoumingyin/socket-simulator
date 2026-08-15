//! 传输层连接回调集合（≡ Node transport hooks）
//!
//! 原误置于 `websocket.rs`（P1-3 迁出）：`TransportHooks` 是各传输层（Ws/Http/
//! Unified/SocketIo）共用的连接回调契约，与具体 WsServer 实现无关。迁出后
//! websocket/http/unified/socketio 与 service_manager 统一从本模块引用。

use std::sync::Arc;

use serde_json::Value;

use crate::backend::types::ClientInfo;

/// 受管服务连接回调集合（由 ServiceManager 注入）
#[derive(Clone)]
pub struct TransportHooks {
    /// 客户端连接：`ClientInfo.id` 为原始 socketId（不含 serverId 前缀）
    pub on_connect: Arc<dyn Fn(ClientInfo) + Send + Sync>,
    /// 收到消息：`(socket_id, event, data)`
    pub on_message: Arc<dyn Fn(String, String, Value) + Send + Sync>,
    /// 客户端断开：`socket_id`
    pub on_disconnect: Arc<dyn Fn(String) + Send + Sync>,
}

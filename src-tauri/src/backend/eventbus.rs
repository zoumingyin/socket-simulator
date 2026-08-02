//! 事件总线（≡ Node EventEmitter）
//!
//! 用 `tokio::sync::broadcast` 解耦各 Manager 与订阅方（管理端 WS hub、日志文件写入等）。
//! 等价于 Node 的 `EventEmitter`，但无回调节点。

use std::collections::HashMap;

use tokio::sync::broadcast;

use crate::backend::types::{ClientInfo, LogEntry, ServerRuntime};

/// 跨模块事件总线
#[derive(Clone)]
pub struct EventBus {
    /// runtime 快照（全量 runtimes map）
    pub runtime_tx: broadcast::Sender<HashMap<String, ServerRuntime>>,
    /// client 全量列表
    pub client_tx: broadcast::Sender<Vec<ClientInfo>>,
    /// 单条日志
    pub log_tx: broadcast::Sender<LogEntry>,
}

impl EventBus {
    pub fn new() -> Self {
        let (runtime_tx, _) = broadcast::channel(512);
        let (client_tx, _) = broadcast::channel(512);
        let (log_tx, _) = broadcast::channel(1024);
        Self {
            runtime_tx,
            client_tx,
            log_tx,
        }
    }

    /// 发布运行时快照
    pub fn publish_runtime(&self, runtimes: HashMap<String, ServerRuntime>) {
        let _ = self.runtime_tx.send(runtimes);
    }

    /// 发布客户端全量列表
    pub fn publish_client(&self, clients: Vec<ClientInfo>) {
        let _ = self.client_tx.send(clients);
    }

    /// 发布单条日志
    pub fn publish_log(&self, entry: LogEntry) {
        let _ = self.log_tx.send(entry);
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

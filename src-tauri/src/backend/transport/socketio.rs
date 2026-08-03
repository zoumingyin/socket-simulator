//! SocketIoServer —— 受管服务的 Socket.IO 传输层（≡ Node SocketIoTransport）
//!
//! 基于 `socketioxide` 0.18（hyper 1.0 / http 1.0，兼容官方 JS 客户端 v3+）。每个
//! `ServerConfig` 独立 `TcpListener` 做 accept loop，把每个连接升级为 Socket.IO 会话。
//!
//! 注意：项目 REST/管理端使用的是 `axum` 0.7（底层 http 0.2 / hyper 0.14），而
//! `socketioxide` 0.18 要求 http 1.0 / hyper 1.9，二者 `Layer`/`Service` 的 `Request`
//! 类型（`http::Request`）版本不兼容，无法用 `axum::Router().layer(layer)` 组合。因此这里
//! 直接驱动 `socketioxide` 原生暴露的 `hyper` 1.0 服务（`SocketIoService`），配合 `hyper-util`
//! 的 `TokioIo` / `TokioExecutor` 做连接级 serve，避免引入对既有 axum 0.7 栈的改动。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;
use socketioxide::extract::{Data, Event, SocketRef};
use socketioxide::socket::DisconnectReason;
use socketioxide::SocketIo;
use tokio::net::TcpListener;
use tokio::task::AbortHandle;

use crate::backend::constants::*;
use crate::backend::error::BackendError;
use crate::backend::net::port_release::release_port;
use crate::backend::transport::Transport;
use crate::backend::transport::websocket::TransportHooks;
use crate::backend::types::now_rfc3339;
use crate::backend::types::*;

/// 受管服务 Socket.IO 服务端
pub struct SocketIoServer {
    cfg: ServerConfig,
    /// 系统设置（与 WsServer 保持结构一致；Socket.IO 侧的 IP 过滤等能力后续可复用）
    #[allow(dead_code)]
    sys: SystemSettings,
    hooks: TransportHooks,
    /// `SocketIo` 句柄（持有与 `SocketIoService` 共享的 `Client`），用于广播
    io: Mutex<Option<SocketIo>>,
    /// raw socketId → 该连接的 `SocketRef`（用于定向 send / disconnect）
    clients: Arc<Mutex<HashMap<String, SocketRef>>>,
    /// 运行状态；用 `Arc` 包裹以便在 accept loop 任务中共享观察
    running: Arc<AtomicBool>,
    /// accept loop 任务的终止句柄
    abort: Mutex<Option<AbortHandle>>,
}

impl SocketIoServer {
    /// 构造
    pub fn new(cfg: ServerConfig, sys: SystemSettings, hooks: TransportHooks) -> Self {
        Self {
            cfg,
            sys,
            hooks,
            io: Mutex::new(None),
            clients: Arc::new(Mutex::new(HashMap::new())),
            running: Arc::new(AtomicBool::new(false)),
            abort: Mutex::new(None),
        }
    }
}

#[async_trait::async_trait]
impl Transport for SocketIoServer {
    async fn start(&self) -> Result<(), BackendError> {
        if self.running.load(Ordering::SeqCst) {
            return Ok(());
        }

        // 1) 绑定监听（端口冲突时释放占用进程后重试，与 WsServer 行为一致）
        let addr = (self.cfg.ip.as_str(), self.cfg.port as u16);
        let listener = match TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                eprintln!(
                    "[SocketIoServer] 端口 {} 被占用，尝试释放后重试",
                    self.cfg.port
                );
                release_port(self.cfg.port as u16);
                tokio::time::sleep(Duration::from_millis(PORT_RELEASE_RETRY_DELAY_MS)).await;
                TcpListener::bind(addr).await?
            }
            Err(e) => return Err(e.into()),
        };

        // 2) 创建 Socket.IO 层与服务（standalone：非 socket.io 请求返回 404）
        let (service, io) = SocketIo::new_svc();

        // 3) 取好连接处理器闭包所需捕获的克隆值（均为 Clone / Copy）
        let clients = self.clients.clone();
        let on_connect = self.hooks.on_connect.clone();
        let on_message = self.hooks.on_message.clone();
        let on_disconnect = self.hooks.on_disconnect.clone();
        let server_id = self.cfg.id.clone();
        let protocol = self.cfg.protocol;
        let ip = self.cfg.ip.clone();

        // 4) 注册默认命名空间 "/"，每个 socket 连入触发一次
        io.ns("/", async move |s: SocketRef| {
            let sid = s.id.to_string();
            clients.lock().unwrap().insert(sid.clone(), s.clone());

            let info = ClientInfo {
                id: sid.clone(),
                server_id: server_id.clone(),
                socket_id: sid.clone(),
                ip_address: ip.clone(),
                connected_at: now_rfc3339(),
                last_activity_at: now_rfc3339(),
                protocol,
                status: ClientStatus::Connected,
                ..Default::default()
            };
            (on_connect)(info);

            // 断开处理器：清理客户端表并回调上层（每个闭包独立持有 sid 副本）
            let dc_sid = sid.clone();
            s.on_disconnect(async move |_s: SocketRef, _reason: DisconnectReason| {
                clients.lock().unwrap().remove(&dc_sid);
                (on_disconnect)(dc_sid.clone());
            });

            // 兜底/通配事件处理器：无精确匹配的事件会落到这里
            let fb_sid = sid.clone();
            s.on_fallback(
                async move |_s: SocketRef, Event(event): Event, Data(data): Data<Value>| {
                    (on_message)(fb_sid.clone(), event, data);
                },
            );
        });

        // 5) 保存 io 句柄（供广播使用），并标记运行中
        *self.io.lock().unwrap() = Some(io);
        self.running.store(true, Ordering::SeqCst);

        // 6) 启动 accept loop：每个 TCP 连接交给一个 Socket.IO 服务实例处理
        let running = self.running.clone();
        let handle = tokio::spawn(async move {
            loop {
                if !running.load(Ordering::SeqCst) {
                    break;
                }
                let (stream, _peer) = match listener.accept().await {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("[SocketIoServer] accept 失败: {}", e);
                        continue;
                    }
                };
                let conn_svc = service.clone();
                tokio::spawn(async move {
                    let io = hyper_util::rt::TokioIo::new(stream);
                    let builder = hyper_util::server::conn::auto::Builder::new(
                        hyper_util::rt::TokioExecutor::new(),
                    );
                    if let Err(e) = builder.serve_connection(io, conn_svc).await {
                        eprintln!("[SocketIoServer] 连接处理错误: {}", e);
                    }
                });
            }
        });
        *self.abort.lock().unwrap() = Some(handle.abort_handle());

        Ok(())
    }

    async fn stop(&self) -> Result<(), BackendError> {
        self.running.store(false, Ordering::SeqCst);
        // 终止 accept loop 任务（连接级任务在其 TCP 连接断开后自行结束）
        if let Some(a) = self.abort.lock().unwrap().take() {
            a.abort();
        }
        // 清空客户端表（纯簿记；连接级任务随后释放）
        self.clients.lock().unwrap().clear();
        // drop io 句柄；底层 Client 随 SocketIoService 克隆引用一并释放
        self.io.lock().unwrap().take();
        Ok(())
    }

    async fn send(&self, client_id: &str, event: &str, data: Value) -> Result<(), BackendError> {
        // 先取出 SocketRef 再调用（避免持有 std::sync::MutexGuard 跨调用）
        let s = self.clients.lock().unwrap().get(client_id).cloned();
        if let Some(s) = s {
            s.emit(event, &data)
                .map_err(|e| BackendError::Internal(e.to_string()))?;
        }
        Ok(())
    }

    async fn broadcast(
        &self,
        event: &str,
        data: Value,
        target_ids: Option<Vec<String>>,
    ) -> Result<(), BackendError> {
        // None 或空列表 → 全量广播；否则按 target_ids 定向发送
        if target_ids.as_ref().map_or(true, |ids| ids.is_empty()) {
            // 全量广播：通过 io 句柄广播给该服务下所有在线连接
            // 注意：BroadcastOperators::emit 返回 Future，需 await；
            // 先把 io 句柄 clone 出锁，避免持有 std::sync::MutexGuard 跨 await（Guard 非 Send）
            let io = self.io.lock().unwrap().as_ref().cloned();
            if let Some(io) = io {
                io.broadcast()
                    .emit(event, &data)
                    .await
                    .map_err(|e| BackendError::Internal(e.to_string()))?;
            }
        } else {
            let ids = target_ids.unwrap();
            for id in ids {
                // 取出发送端后再调用，避免持有 MutexGuard 跨调用
                    let s = self.clients.lock().unwrap().get(&id).cloned();
                    if let Some(s) = s {
                        s.emit(event, &data.clone())
                            .map_err(|e| BackendError::Internal(e.to_string()))?;
                    }
                }
            }
        Ok(())
    }

    async fn disconnect_client(&self, client_id: &str) -> Result<(), BackendError> {
        // 取出 SocketRef 后调用 disconnect（消费 self），避免持有 MutexGuard
        let s = self.clients.lock().unwrap().remove(client_id);
        if let Some(s) = s {
            let _ = s.disconnect();
        }
        Ok(())
    }
}

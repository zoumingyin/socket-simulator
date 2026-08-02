//! WsServer —— 受管服务的纯 WebSocket 传输层（≡ Node WebSocketTransport）
//!
//! 每个 `ServerConfig` 独立 `TcpListener` 做 accept loop；握手期做 IP 黑白名单过滤（P1-3）
//! 与最大连接数强制（P1/f）；可选 WSS（仅受管服务，证书缺失降级纯 WS，P1-2）。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};

use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_rustls::server::TlsStream;
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::accept_async;

use crate::backend::constants::*;
use crate::backend::error::BackendError;
use crate::backend::net::port_release::release_port;
use crate::backend::transport::Transport;
use crate::backend::types::*;

use nanoid::nanoid;

/// 受管服务连接回调集合（由 ServiceManager 注入）
pub struct TransportHooks {
    /// 客户端连接：`ClientInfo.id` 为原始 socketId（不含 serverId 前缀）
    pub on_connect: Arc<dyn Fn(ClientInfo) + Send + Sync>,
    /// 收到消息：`(socket_id, event, data)`
    pub on_message: Arc<dyn Fn(String, String, Value) + Send + Sync>,
    /// 客户端断开：`socket_id`
    pub on_disconnect: Arc<dyn Fn(String) + Send + Sync>,
}

/// 受管服务 WebSocket 服务端
pub struct WsServer {
    cfg: ServerConfig,
    sys: SystemSettings,
    /// raw socketId → 该连接的外发消息发送端
    clients: Arc<Mutex<HashMap<String, mpsc::Sender<Message>>>>,
    running: Arc<AtomicBool>,
    hooks: TransportHooks,
    tls: Option<TlsAcceptor>,
    self_ref: Weak<WsServer>,
}

impl WsServer {
    /// 构造（通过 `Arc::new_cyclic` 注入 self 弱引用，供 accept loop 取 Arc<Self>）
    pub fn new(
        cfg: ServerConfig,
        sys: SystemSettings,
        hooks: TransportHooks,
        weak: Weak<WsServer>,
    ) -> Self {
        let tls = if cfg.wss_enabled {
            load_tls_acceptor(&cfg)
        } else {
            None
        };
        if cfg.wss_enabled && tls.is_none() {
            eprintln!(
                "[WsServer] 服务 {} 启用 WSS 但证书缺失，降级纯 WS",
                cfg.id
            );
        }
        Self {
            cfg,
            sys,
            clients: Arc::new(Mutex::new(HashMap::new())),
            running: Arc::new(AtomicBool::new(false)),
            hooks,
            tls,
            self_ref: weak,
        }
    }

    pub fn cfg(&self) -> &ServerConfig {
        &self.cfg
    }

    /// IP 黑白名单过滤（P1-3）
    fn allow_ip(&self, ip: &str) -> bool {
        let wl = &self.sys.ip_access.whitelist;
        let bl = &self.sys.ip_access.blacklist;
        if bl.iter().any(|b| b == ip) {
            return false;
        }
        if !wl.is_empty() && !wl.iter().any(|w| w == ip) {
            return false;
        }
        true
    }

    /// accept loop：绑定完成后由 `start` 派生
    async fn run_loop(self: Arc<Self>, listener: TcpListener) {
        loop {
            if !self.running.load(Ordering::SeqCst) {
                break;
            }
            let accept = listener.accept().await;
            let (stream, addr) = match accept {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("[WsServer] accept 失败: {}", e);
                    continue;
                }
            };

            // 握手前：IP 过滤 + 最大连接数
            let ip = addr.ip().to_string();
            if !self.allow_ip(&ip) {
                eprintln!("[WsServer] 拒绝连接（IP 黑名单/非白名单）: {}", ip);
                continue;
            }
            {
                let cur = self.clients.lock().unwrap().len();
                if cur >= self.sys.max_connections_per_server as usize {
                    eprintln!(
                        "[WsServer] 超过最大连接数 {}，拒绝新连接",
                        self.sys.max_connections_per_server
                    );
                    continue;
                }
            }

            let raw_id = nanoid!(16);
            let tls = self.tls.clone();
            if let Some(acceptor) = tls {
                let tls_stream = match acceptor.accept(stream).await {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("[WsServer] TLS 握手失败: {}", e);
                        continue;
                    }
                };
                let ws = match accept_async(tls_stream).await {
                    Ok(w) => w,
                    Err(e) => {
                        eprintln!("[WsServer] WS 握手失败: {}", e);
                        continue;
                    }
                };
                tokio::spawn(self.clone().handle_conn::<TlsStream<TcpStream>>(raw_id, ws, ip));
            } else {
                let ws = match accept_async(stream).await {
                    Ok(w) => w,
                    Err(e) => {
                        eprintln!("[WsServer] WS 握手失败: {}", e);
                        continue;
                    }
                };
                tokio::spawn(self.clone().handle_conn::<TcpStream>(raw_id, ws, ip));
            }
        }
    }

    /// 单连接处理：读取入站消息、转发外发消息、生命周期清理
    async fn handle_conn<S>(self: Arc<Self>, raw_id: String, ws: WebSocketStream<S>, ip: String)
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let (mut write, mut read) = ws.split();
        let (out_tx, mut out_rx) = mpsc::channel::<Message>(64);
        self.clients.lock().unwrap().insert(raw_id.clone(), out_tx);

        // 通知连接
        let now = now_rfc3339();
        let info = ClientInfo {
            id: raw_id.clone(),
            server_id: self.cfg.id.clone(),
            socket_id: raw_id.clone(),
            ip_address: ip.clone(),
            connected_at: now.clone(),
            last_activity_at: now,
            protocol: self.cfg.protocol,
            status: ClientStatus::Connected,
            group: None,
            group_name: None,
            metadata: None,
        };
        (self.hooks.on_connect)(info);

        loop {
            tokio::select! {
                incoming = read.next() => {
                    match incoming {
                        Some(Ok(Message::Text(text))) => {
                            match serde_json::from_str::<WsFrame>(&text) {
                                Ok(frame) => (self.hooks.on_message)(raw_id.clone(), frame.event, frame.data),
                                Err(_) => (self.hooks.on_message)(
                                    raw_id.clone(),
                                    "message".to_string(),
                                    serde_json::json!({ "raw": text }),
                                ),
                            }
                        }
                        Some(Ok(Message::Binary(bin))) => {
                            let text = String::from_utf8_lossy(&bin).to_string();
                            (self.hooks.on_message)(
                                raw_id.clone(),
                                "message".to_string(),
                                serde_json::json!({ "raw": text }),
                            );
                        }
                        Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => {
                            // tungstenite 会自动回复 ping，无需处理
                        }
                        Some(Ok(Message::Close(_))) | None => break,
                        Some(Err(e)) => {
                            eprintln!("[WsServer] 连接 {} 读错误: {}", raw_id, e);
                            break;
                        }
                        _ => {}
                    }
                }
                outgoing = out_rx.recv() => {
                    match outgoing {
                        Some(msg) => {
                            if write.send(msg).await.is_err() {
                                break;
                            }
                        }
                        None => break, // 发送端被丢弃（如 disconnect_client 移除）
                    }
                }
            }
        }

        // 清理
        self.clients.lock().unwrap().remove(&raw_id);
        (self.hooks.on_disconnect)(raw_id.clone());
    }
}

#[async_trait::async_trait]
impl Transport for WsServer {
    fn protocol(&self) -> ProtocolType {
        self.cfg.protocol
    }

    async fn start(&self) -> Result<(), BackendError> {
        if self.running.load(Ordering::SeqCst) {
            return Ok(());
        }
        let addr = (self.cfg.ip.as_str(), self.cfg.port as u16);
        let listener = match TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                eprintln!("[WsServer] 端口 {} 被占用，尝试释放后重试", self.cfg.port);
                release_port(self.cfg.port as u16);
                tokio::time::sleep(std::time::Duration::from_millis(PORT_RELEASE_RETRY_DELAY_MS)).await;
                TcpListener::bind(addr).await?
            }
            Err(e) => return Err(e.into()),
        };
        self.running.store(true, Ordering::SeqCst);
        let this = self
            .self_ref
            .upgrade()
            .ok_or_else(|| BackendError::Internal("WsServer self 弱引用缺失".into()))?;
        tokio::spawn(this.run_loop(listener));
        Ok(())
    }

    async fn stop(&self) -> Result<(), BackendError> {
        self.running.store(false, Ordering::SeqCst);
        // 清空客户端连接，让各连接任务因 out_rx 关闭而结束
        self.clients.lock().unwrap().clear();
        Ok(())
    }

    async fn send(&self, client_id: &str, event: &str, data: Value) -> Result<(), BackendError> {
        let msg = Message::Text(
            serde_json::to_string(&WsFrame {
                event: event.to_string(),
                data,
            })
            .unwrap_or_default(),
        );
        // 在锁作用域内取出发送端，确保 MutexGuard 不跨 .await（std::sync::MutexGuard 非 Send）
        let tx = self.clients.lock().unwrap().get(client_id).cloned();
        if let Some(tx) = tx {
            let _ = tx.send(msg).await;
        }
        Ok(())
    }

    async fn broadcast(
        &self,
        event: &str,
        data: Value,
        target_ids: Option<Vec<String>>,
    ) -> Result<(), BackendError> {
        let payload = Message::Text(
            serde_json::to_string(&WsFrame {
                event: event.to_string(),
                data,
            })
            .unwrap_or_default(),
        );
        let targets: Vec<String> = match target_ids {
            Some(ids) => ids,
            None => {
                let guard = self.clients.lock().unwrap();
                guard.keys().cloned().collect()
            }
        };
        for id in targets {
            // 取出发送端后再 await，避免持有 std::sync::MutexGuard 跨 await
            let tx = self.clients.lock().unwrap().get(&id).cloned();
            if let Some(tx) = tx {
                let _ = tx.send(payload.clone()).await;
            }
        }
        Ok(())
    }

    async fn disconnect_client(&self, client_id: &str) -> Result<(), BackendError> {
        // 取出发送端后再 await，避免持有 std::sync::MutexGuard 跨 await
        let tx = self.clients.lock().unwrap().remove(client_id);
        if let Some(tx) = tx {
            let _ = tx.send(Message::Close(None)).await;
        }
        Ok(())
    }

    fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

/// 加载 TLS 证书，构造 TlsAcceptor；失败返回 None（调用方降级纯 WS）
fn load_tls_acceptor(cfg: &ServerConfig) -> Option<TlsAcceptor> {
    let cert_path = cfg.cert_path.as_ref()?;
    let key_path = cfg.key_path.as_ref()?;
    let cert_file = std::fs::File::open(cert_path).ok()?;
    let key_file = std::fs::File::open(key_path).ok()?;
    let mut cert_reader = std::io::BufReader::new(cert_file);
    let mut key_reader = std::io::BufReader::new(key_file);

    let certs = rustls_pemfile::certs(&mut cert_reader)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    let key = rustls_pemfile::private_key(&mut key_reader).ok()??;

    let server_config = tokio_rustls::rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .ok()?;
    Some(TlsAcceptor::from(Arc::new(server_config)))
}

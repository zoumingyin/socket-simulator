//! WsServer —— 受管服务的纯 WebSocket 传输层（≡ Node WebSocketTransport）
//!
//! 每个 `ServerConfig` 独立 `TcpListener` 做 accept loop；握手期做 IP 黑白名单过滤（P1-3）
//! 与最大连接数强制（P1/f）；可选 WSS（仅受管服务，证书缺失降级纯 WS，P1-2）。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};

use futures_util::StreamExt; // 提供 `WebSocketStream::split()`（handle_conn 拆流用）；发送由 pump_ws 内部处理
use serde_json::Value;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_rustls::server::TlsStream;
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::accept_async;

use crate::backend::constants::*;
use crate::backend::error::BackendError;
use crate::backend::net::bind::bind_with_release;
use crate::backend::transport::hooks::TransportHooks;
use crate::backend::transport::ws_connection::{
    frame_to_text, pump_ws, TungsteniteAdapter, WireMsg, WsClientRegistry,
};
use crate::backend::transport::Transport;
use crate::backend::types::*;

use nanoid::nanoid;

/// 受管服务 WebSocket 服务端
pub struct WsServer {
    cfg: ServerConfig,
    sys: SystemSettings,
    /// raw socketId → 该连接的外发消息发送端（统一为 WireMsg）
    clients: WsClientRegistry,
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

    /// IP 黑白名单过滤（统一实现见 `crate::backend::net::ip_access`）
    fn allow_ip(&self, ip: &str) -> bool {
        crate::backend::net::ip_access::allow_ip(
            &self.sys.ip_access.whitelist,
            &self.sys.ip_access.blacklist,
            ip,
        )
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

    /// 单连接处理：拆流 → 注册外发通道 → 交给 `pump_ws`（F-1 收敛后的唯一实现）。
    /// 连接生命周期/消息路由逻辑全部在 `pump_ws` 中，本方法仅负责 tungstenite 握手后的薄封装。
    async fn handle_conn<S>(self: Arc<Self>, raw_id: String, ws: WebSocketStream<S>, ip: String)
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let (write, read) = ws.split();
        let (out_tx, out_rx) = mpsc::channel::<WireMsg>(64);
        self.clients.lock().unwrap().insert(raw_id.clone(), out_tx);

        pump_ws::<TungsteniteAdapter, _, _>(
            read,
            write,
            raw_id.clone(),
            ip,
            self.cfg.clone(),
            self.hooks.clone(),
            out_rx,
        )
        .await;

        // 清理
        self.clients.lock().unwrap().remove(&raw_id);
        (self.hooks.on_disconnect)(raw_id);
    }
}

#[async_trait::async_trait]
impl Transport for WsServer {
    async fn start(&self) -> Result<(), BackendError> {
        if self.running.load(Ordering::SeqCst) {
            return Ok(());
        }
        let listener = bind_with_release(self.cfg.ip.as_str(), self.cfg.port as u16).await?;
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
        let msg = WireMsg::Text(frame_to_text(event, &data));
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
        let payload = WireMsg::Text(frame_to_text(event, &data));
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
            // 先发 Close 帧（优雅关闭），tx 丢弃后 pump 的 out_rx 关闭 → 连接结束
            let _ = tx.send(WireMsg::Close).await;
        }
        Ok(())
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

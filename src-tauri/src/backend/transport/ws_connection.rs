//! WS 连接处理公共实现（F-1：收敛 WsServer 与 UnifiedServer 的 WS 消息泵）
//!
//! 原先 `websocket.rs::handle_conn` 与 `unified.rs::handle_ws_connection` 各有一份
//! 几乎相同的连接循环（读文本/二进制 → 解析 WsFrame → 路由 on_message；转发外发消息；
//! 断线清理），仅 `Message` 类型不同（tungstenite vs axum）。
//!
//! 本模块抽取为泛型 `pump_ws`：通过 `WireAdapter` 适配层屏蔽两套 `Message` 差异，
//! 使「WS 连接生命周期 + 消息路由」成为**唯一实现**。两处 server 仅保留各自监听/
//! 握手/注册表差异（WsServer 走裸 TcpListener + 可选 WSS；Unified 走 axum 共端口）。
//!
//! 监听层不合并的合理性：WsServer 需要裸 TCP（TLS 证书 / 独立端口），Unified 需要
//! axum Router（与 HTTP/Mock 同端口）。二者在「连接到消息泵」这一层已被收敛。

use std::sync::{Arc, Mutex};
use std::collections::HashMap;

use futures_util::{Sink, SinkExt, Stream, StreamExt};
use serde_json::Value;
use tokio::sync::mpsc;

use crate::backend::transport::hooks::TransportHooks;
use crate::backend::types::*;
use crate::backend::types::now_rfc3339;

/// 与具体 WS 库无关的线级消息表达
#[derive(Debug, Clone)]
pub enum WireMsg {
    Text(String),
    Binary(Vec<u8>),
    /// 关闭连接（泵收到后退出循环）
    Close,
    /// 可安全忽略的消息（Ping/Pong 等，由底层库自行处理）
    Noop,
}

/// 线级消息适配：屏蔽 tungstenite 与 axum 的 `Message` 类型差异
pub trait WireAdapter: Send + Sync + 'static {
    type Message: Send + 'static;
    type Error: std::fmt::Display + Send + 'static;
    /// 收：库消息 → WireMsg（`Noop` 表示忽略）
    fn from_wire(msg: Self::Message) -> WireMsg;
    /// 发：WireMsg → 库消息
    fn to_wire(msg: WireMsg) -> Self::Message;
}

/// tungstenite（WsServer）适配
pub struct TungsteniteAdapter;
impl WireAdapter for TungsteniteAdapter {
    type Message = tokio_tungstenite::tungstenite::Message;
    type Error = tokio_tungstenite::tungstenite::Error;
    fn from_wire(msg: Self::Message) -> WireMsg {
        match msg {
            tokio_tungstenite::tungstenite::Message::Text(t) => WireMsg::Text(t.to_string()),
            tokio_tungstenite::tungstenite::Message::Binary(b) => WireMsg::Binary(b.to_vec()),
            tokio_tungstenite::tungstenite::Message::Close(_) => WireMsg::Close,
            _ => WireMsg::Noop,
        }
    }
    fn to_wire(msg: WireMsg) -> Self::Message {
        match msg {
            WireMsg::Text(t) => tokio_tungstenite::tungstenite::Message::Text(t.into()),
            WireMsg::Binary(b) => tokio_tungstenite::tungstenite::Message::Binary(b.into()),
            WireMsg::Close => tokio_tungstenite::tungstenite::Message::Close(None),
            WireMsg::Noop => tokio_tungstenite::tungstenite::Message::Ping(Vec::new().into()),
        }
    }
}

/// axum（UnifiedServer）适配
pub struct AxumAdapter;
impl WireAdapter for AxumAdapter {
    type Message = axum::extract::ws::Message;
    type Error = axum::Error;
    fn from_wire(msg: Self::Message) -> WireMsg {
        match msg {
            axum::extract::ws::Message::Text(t) => WireMsg::Text(t),
            // axum 的 Binary 持有 `Bytes`，统一收敛为 `Vec<u8>`
            axum::extract::ws::Message::Binary(b) => WireMsg::Binary(b.to_vec()),
            axum::extract::ws::Message::Close(_) => WireMsg::Close,
            _ => WireMsg::Noop,
        }
    }
    fn to_wire(msg: WireMsg) -> Self::Message {
        match msg {
            WireMsg::Text(t) => axum::extract::ws::Message::Text(t),
            // `Vec<u8>` → axum 的 `Bytes`
            WireMsg::Binary(b) => axum::extract::ws::Message::Binary(b.into()),
            WireMsg::Close => axum::extract::ws::Message::Close(None),
            WireMsg::Noop => axum::extract::ws::Message::Ping(Vec::new().into()),
        }
    }
}

/// 构建一条 WS 帧的 JSON 文本（send/broadcast/disconnect 共用，消除重复序列化）
pub fn frame_to_text(event: &str, data: &Value) -> String {
    serde_json::to_string(&WsFrame {
        event: event.to_string(),
        data: data.clone(),
    })
    .unwrap_or_default()
}

/// 泛型 WS 连接消息泵（F-1 收敛后的唯一实现）
///
/// - 入站：解析 Text/Binary → 路由 `hooks.on_message`；Close → 退出；Noop → 忽略；
///   读错误/流结束 → 退出。
/// - 出站：`out_rx` 收到 `WireMsg` → 经 `A::to_wire` 转换后写回；通道关闭 → 退出。
/// - 连接建立时回调 `hooks.on_connect`（构造 ClientInfo）；循环退出即视为断开，
///   由调用方负责从注册表移除并回调 `hooks.on_disconnect`。
pub async fn pump_ws<A, R, W>(
    mut read: R,
    mut write: W,
    raw_id: String,
    ip: String,
    cfg: ServerConfig,
    hooks: TransportHooks,
    mut out_rx: mpsc::Receiver<WireMsg>,
) where
    A: WireAdapter,
    R: Stream<Item = Result<A::Message, A::Error>> + Unpin,
    W: Sink<A::Message, Error = A::Error> + Unpin,
{
    // 通知连接
    let now = now_rfc3339();
    let info = ClientInfo {
        id: raw_id.clone(),
        server_id: cfg.id.clone(),
        socket_id: raw_id.clone(),
        ip_address: ip.clone(),
        connected_at: now.clone(),
        last_activity_at: now,
        protocol: cfg.protocol,
        status: ClientStatus::Connected,
        group: None,
        group_name: None,
        metadata: None,
    };
    (hooks.on_connect)(info);

    loop {
        tokio::select! {
            incoming = read.next() => {
                match incoming {
                    Some(Ok(m)) => match A::from_wire(m) {
                        WireMsg::Text(text) => {
                            match serde_json::from_str::<WsFrame>(&text) {
                                Ok(frame) => (hooks.on_message)(
                                    raw_id.clone(),
                                    frame.event,
                                    frame.data,
                                ),
                                Err(_) => (hooks.on_message)(
                                    raw_id.clone(),
                                    "message".to_string(),
                                    serde_json::json!({ "raw": text }),
                                ),
                            }
                        }
                        WireMsg::Binary(bin) => {
                            let text = String::from_utf8_lossy(&bin).to_string();
                            (hooks.on_message)(
                                raw_id.clone(),
                                "message".to_string(),
                                serde_json::json!({ "raw": text }),
                            );
                        }
                        WireMsg::Close => break,
                        WireMsg::Noop => {}
                    },
                    Some(Err(e)) => {
                        eprintln!("[ws_connection] 连接 {} 读错误: {}", raw_id, e);
                        break;
                    }
                    None => break,
                }
            }
            outgoing = out_rx.recv() => {
                match outgoing {
                    Some(wire) => {
                        if write.send(A::to_wire(wire)).await.is_err() {
                            break;
                        }
                    }
                    None => break,
                }
            }
        }
    }
}

/// 注册表类型别名：两处 server 的 WS 客户端表统一为 `mpsc::Sender<WireMsg>`
pub type WsClientRegistry = Arc<Mutex<HashMap<String, mpsc::Sender<WireMsg>>>>;

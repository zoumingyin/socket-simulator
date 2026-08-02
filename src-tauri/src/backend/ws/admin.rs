//! 管理端 WebSocket 通道（≡ Node Socket.IO `/admin/socket.io`）
//!
//! 路径：`/admin/ws`。连接建立后推送初始快照（runtime / client / log_batch），
//! 随后订阅 EventBus 把运行时/客户端/日志变更实时转发给前端；每隔 `HEARTBEAT_INTERVAL_MS`
//! 发送 `heartbeat`，前端回 `heartbeat_ack` 以维持连接（超过 `HEARTBEAT_TIMEOUT_MS` 判定僵尸）。

use std::time::{Duration, Instant};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use futures_util::{SinkExt, StreamExt};

use crate::backend::constants::*;
use crate::backend::state::AppState;
use crate::backend::types::*;

/// axum 路由处理函数：升级 HTTP 为 WebSocket
pub async fn admin_ws(ws: WebSocketUpgrade, State(b): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| handle_admin_socket(socket, b))
}

async fn handle_admin_socket(socket: WebSocket, backend: AppState) {
    let (mut sender, mut receiver) = socket.split();

    // 初始数据快照
    let runtimes = backend.services.get_runtimes();
    send_frame(
        &mut sender,
        EVT_RUNTIME_UPDATE,
        serde_json::to_value(runtimes).unwrap_or(serde_json::Value::Null),
    )
    .await;
    let clients = backend.clients.list();
    send_frame(
        &mut sender,
        EVT_CLIENT_UPDATE,
        serde_json::to_value(clients).unwrap_or(serde_json::Value::Null),
    )
    .await;
    let logs = backend.logs.get_entries_last(LOG_BATCH_INITIAL);
    send_frame(
        &mut sender,
        EVT_LOG_BATCH,
        serde_json::to_value(logs).unwrap_or(serde_json::Value::Null),
    )
    .await;

    // 订阅 EventBus
    let mut rt_rx = backend.event_bus.runtime_tx.subscribe();
    let mut cl_rx = backend.event_bus.client_tx.subscribe();
    let mut log_rx = backend.event_bus.log_tx.subscribe();

    let mut heartbeat = tokio::time::interval(Duration::from_millis(HEARTBEAT_INTERVAL_MS));
    let mut last_ack = Instant::now();

    loop {
        tokio::select! {
            incoming = receiver.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(frame) = serde_json::from_str::<WsFrame>(&text) {
                            if frame.event == EVT_HEARTBEAT_ACK {
                                last_ack = Instant::now();
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
            Ok(rt) = rt_rx.recv() => {
                send_frame(
                    &mut sender,
                    EVT_RUNTIME_UPDATE,
                    serde_json::to_value(rt).unwrap_or(serde_json::Value::Null),
                )
                .await;
            }
            Ok(cl) = cl_rx.recv() => {
                send_frame(
                    &mut sender,
                    EVT_CLIENT_UPDATE,
                    serde_json::to_value(cl).unwrap_or(serde_json::Value::Null),
                )
                .await;
            }
            Ok(entry) = log_rx.recv() => {
                send_frame(
                    &mut sender,
                    EVT_LOG_UPDATE,
                    serde_json::to_value(entry).unwrap_or(serde_json::Value::Null),
                )
                .await;
            }
            _ = heartbeat.tick() => {
                if last_ack.elapsed() > Duration::from_millis(HEARTBEAT_TIMEOUT_MS) {
                    break;
                }
                send_frame(&mut sender, EVT_HEARTBEAT, serde_json::json!({})).await;
            }
        }
    }
}

/// 发送一帧 WS 消息
async fn send_frame(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    event: &str,
    data: serde_json::Value,
) {
    let frame = WsFrame {
        event: event.to_string(),
        data,
    };
    if let Ok(text) = serde_json::to_string(&frame) {
        let _ = sender.send(Message::Text(text)).await;
    }
}

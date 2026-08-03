//! 端到端生命周期集成测试（≡ Node "新建并启动 SocketIO/WebSocket 服务供外部连接"）
//!
//! 这些测试验证：通过 `ServiceManager` 注册并启动一个 `websocket` / `socket.io` 服务后，
//! 后端确实创建了可监听的传输层实例，且**外部客户端能够真实连入**，连接数 / 消息计数会
//! 随连接与收发而更新（等价于服务管理界面"新增服务 → 启动"后的真实行为）。

#![cfg(test)]

use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use crate::backend::eventbus::EventBus;
use crate::backend::managers::client_manager::ClientManager;
use crate::backend::managers::config_manager::ConfigManager;
use crate::backend::managers::log_manager::LogManager;
use crate::backend::managers::service_manager::ServiceManager;
use crate::backend::types::*;

/// 在临时目录构造一个完整 `ServiceManager`（含 Config/Log/Client/EventBus）
fn setup() -> (Arc<ServiceManager>, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("ssm_life_{}", uuid::Uuid::new_v4()));
    let _ = std::fs::create_dir_all(&dir);
    let cm = Arc::new(ConfigManager::new(dir.clone()));
    cm.init();
    let bus = EventBus::new();
    let log_m = Arc::new(LogManager::new(dir.join("logs"), bus.clone()));
    let client_m = Arc::new(ClientManager::new());
    let sm = Arc::new(ServiceManager::new(
        cm,
        log_m,
        client_m,
        bus,
    ));
    (sm, dir)
}

/// 申请一个当前空闲的 TCP 端口（bind :0 取系统分配端口后释放）
async fn free_port() -> u32 {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    drop(l);
    port as u32
}

/// 读取下一条文本消息（跳过 ping/pong 等控制帧）
async fn read_ws_text<S>(stream: &mut S) -> String
where
    S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    loop {
        match stream.next().await {
            Some(Ok(Message::Text(t))) => return t,
            Some(Ok(Message::Binary(b))) => return String::from_utf8_lossy(&b).to_string(),
            Some(Ok(_)) => continue,
            Some(Err(e)) => panic!("ws read error: {e}"),
            None => panic!("ws stream ended unexpectedly"),
        }
    }
}

#[tokio::test]
async fn ws_service_accepts_external_client_and_relays() {
    let (sm, _tmp) = setup();
    let port = free_port().await;
    let cfg = ServerConfig {
        id: "ws1".to_string(),
        name: "ws-e2e".to_string(),
        ip: "127.0.0.1".to_string(),
        port,
        protocol: ProtocolType::Websocket,
        ..Default::default()
    };
    sm.register_server(cfg.clone());
    sm.config.save_servers(vec![cfg]);
    sm.start("ws1".to_string()).await.expect("ws start");

    // 外部客户端真实连入
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/"))
        .await
        .expect("external ws client connect");

    // 等连接回调登记
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        sm.get_runtimes().get("ws1").unwrap().client_count,
        1,
        "外部客户端连入后 client_count 应为 1"
    );

    // 客户端发送一帧 → 后端应计数 received_messages
    ws.send(Message::Text(
        serde_json::json!({ "event": "ping", "data": { "x": 1 } }).to_string(),
    ))
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        sm.get_runtimes().get("ws1").unwrap().received_messages >= 1,
        "后端应收到客户端消息并累加 received_messages"
    );

    // 后端向该服务广播 → 客户端应收到帧
    sm.broadcast("ws1", "server-event", serde_json::json!({ "hi": 2 }))
        .await
        .unwrap();
    let got = read_ws_text(&mut ws).await;
    assert!(
        got.contains("server-event"),
        "客户端应收到后端广播帧，实际: {got}"
    );

    sm.stop("ws1").await.unwrap();
}

#[tokio::test]
async fn sio_service_accepts_external_client_and_relays() {
    let (sm, _tmp) = setup();
    let port = free_port().await;
    let cfg = ServerConfig {
        id: "sio1".to_string(),
        name: "sio-e2e".to_string(),
        ip: "127.0.0.1".to_string(),
        port,
        protocol: ProtocolType::SocketIo,
        ..Default::default()
    };
    sm.register_server(cfg.clone());
    sm.config.save_servers(vec![cfg]);
    sm.start("sio1".to_string()).await.expect("sio start");

    // 外部 Socket.IO 客户端（走 websocket transport 的 engine.io 握手）
    let url = format!("ws://127.0.0.1:{port}/socket.io/?EIO=4&transport=websocket");
    let (mut ws, _) = tokio_tungstenite::connect_async(url)
        .await
        .expect("external sio client connect");

    // 1) 读取 engine.io 打开包 `0{...}`
    let open = read_ws_text(&mut ws).await;
    assert!(
        open.starts_with('0'),
        "应收到 engine.io 打开包，实际: {open}"
    );

    // 2) 发送 socket.io 连接包 `40`
    ws.send(Message::Text("40".to_string())).await.unwrap();
    // 读取服务端回执（可能是一个或多个帧，含 `40` 连接确认）
    let _ack = read_ws_text(&mut ws).await;

    // 等连接回调登记
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(
        sm.get_runtimes().get("sio1").unwrap().client_count,
        1,
        "外部 Socket.IO 客户端连入后 client_count 应为 1"
    );

    // 3) 发送自定义事件 `42["myevent",{...}]`
    ws.send(Message::Text(r#"42["myevent",{"hello":"world"}]"#.to_string()))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(
        sm.get_runtimes().get("sio1").unwrap().received_messages >= 1,
        "后端应收到 Socket.IO 事件并累加 received_messages"
    );

    sm.stop("sio1").await.unwrap();
}

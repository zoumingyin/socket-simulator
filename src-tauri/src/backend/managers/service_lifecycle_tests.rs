//! 端到端生命周期集成测试（≡ Node "新建并启动 SocketIO/WebSocket 服务供外部连接"）
//!
//! 这些测试验证：通过 `ServiceManager` 注册并启动一个 `websocket` / `socket.io` 服务后，
//! 后端确实创建了可监听的传输层实例，且**外部客户端能够真实连入**，连接数 / 消息计数会
//! 随连接与收发而更新（等价于服务管理界面"新增服务 → 启动"后的真实行为）。

#![cfg(test)]

use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;

use crate::backend::eventbus::EventBus;
use crate::backend::managers::client_manager::ClientManager;
use crate::backend::managers::config_manager::ConfigManager;
use crate::backend::managers::log_manager::LogManager;
use crate::backend::managers::service_manager::ServiceManager;
use crate::backend::mock::MockManager;
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

/// 原始 HTTP 请求（TCP 直写；返回 `(状态码, body)`）
async fn http_req(port: u32, method: &str, path: &str, body: Option<&str>) -> (u16, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", port as u16))
        .await
        .expect("http tcp connect");
    let body = body.unwrap_or("");
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(req.as_bytes()).await.expect("write http req");
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.expect("read http resp");
    let text = String::from_utf8_lossy(&buf).to_string();
    let status = text
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let body_part = text.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
    (status, body_part)
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

// ==================== P1-1 行为矩阵（T3 / T4 / T6） ====================
// 验收：四大协议经适配器注册表（P0-5）创建后，行为与 v2 一致。
// 已有覆盖：T1 WS（ws_service_accepts_external_client_and_relays）、T2 SIO
// （sio_service_accepts_external_client_and_relays）。本批补齐：
// T3 HTTP（inbound + SSE）、T4 共端口 Mock（命中/未命中 + WS 共存）、T6 Mock customPort。

/// T3：HTTP 服务 inbound（POST /{event}）+ SSE（GET /stream 广播推送）
#[tokio::test]
async fn http_service_accepts_inbound_and_sse() {
    let (sm, _tmp) = setup();
    let port = free_port().await;
    let cfg = ServerConfig {
        id: "http1".to_string(),
        name: "http-e2e".to_string(),
        ip: "127.0.0.1".to_string(),
        port,
        protocol: ProtocolType::Http,
        ..Default::default()
    };
    sm.register_server(cfg.clone());
    sm.config.save_servers(vec![cfg]);
    sm.start("http1".to_string()).await.expect("http start");

    // 1) inbound：POST /myEvent → 后端 on_message 收到并计数
    let (status, body) = http_req(port, "POST", "/myEvent", Some(r#"{"a":1}"#)).await;
    assert_eq!(status, 200, "inbound 应返回 200，body={body}");
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        sm.get_runtimes().get("http1").unwrap().received_messages >= 1,
        "inbound 消息应累加 received_messages"
    );

    // 2) SSE：GET /stream 建立长连接并登记为客户端
    let mut sse = TcpStream::connect(("127.0.0.1", port as u16))
        .await
        .expect("sse connect");
    let req = format!(
        "GET /stream HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAccept: text/event-stream\r\n\r\n"
    );
    sse.write_all(req.as_bytes()).await.expect("write sse req");
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(
        sm.get_runtimes().get("http1").unwrap().client_count >= 1,
        "SSE 客户端应登记为 client"
    );

    // 3) 先消费响应头（chunked 流首帧通常只有 header）
    let mut header_buf = [0u8; 1024];
    let hn = tokio::time::timeout(Duration::from_secs(2), sse.read(&mut header_buf))
        .await
        .expect("sse header 超时")
        .expect("sse header io");
    let header = String::from_utf8_lossy(&header_buf[..hn]).to_string();
    assert!(
        header.contains("200 OK") && header.contains("text/event-stream"),
        "SSE 响应头应为 200 + text/event-stream，实际: {header}"
    );

    // 4) 广播 → SSE 流应收到 data 帧
    sm.broadcast("http1", "sse-evt", serde_json::json!({ "n": 7 }))
        .await
        .unwrap();
    let mut buf = [0u8; 4096];
    let n = tokio::time::timeout(Duration::from_secs(3), sse.read(&mut buf))
        .await
        .expect("sse data 超时")
        .expect("sse data io");
    let text = String::from_utf8_lossy(&buf[..n]).to_string();
    assert!(
        text.contains("data:") || text.contains("sse-evt"),
        "SSE 应收到广播数据，实际: {text}"
    );

    sm.stop("http1").await.unwrap();
}

/// T4：共端口模式（Unified）—— Mock 规则命中/未命中 + WS 升级共存
#[tokio::test]
async fn unified_ws_with_mock_hits_and_misses() {
    let (sm, _tmp) = setup();
    let port = free_port().await;
    let cfg = ServerConfig {
        id: "un1".to_string(),
        name: "unified-e2e".to_string(),
        ip: "127.0.0.1".to_string(),
        port,
        protocol: ProtocolType::Websocket,
        mock_enabled: true,
        mock_rules: vec![MockRule {
            id: "r1".to_string(),
            name: "get users".to_string(),
            method: HttpMethod::Get,
            path_pattern: "/users".to_string(),
            response_status_code: 200,
            response_body: r#"{"users":[1,2]}"#.to_string(),
            enabled: true,
            ..Default::default()
        }],
        ..Default::default()
    };
    sm.register_server(cfg.clone());
    sm.config.save_servers(vec![cfg]);
    sm.start("un1".to_string()).await.expect("unified start");

    // Mock 命中：规则响应
    let (status, body) = http_req(port, "GET", "/users", None).await;
    assert_eq!(status, 200);
    assert!(
        body.contains("users"),
        "命中规则应返回规则响应体，body={body}"
    );
    // Mock 未命中：默认响应
    let (s2, b2) = http_req(port, "GET", "/nope", None).await;
    assert_eq!(s2, 200);
    assert!(
        b2.contains("message"),
        "未命中应返回默认响应体，body={b2}"
    );

    // WS 升级仍可用（与 Mock 共端口互不干扰）
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/"))
        .await
        .expect("unified ws connect");
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        sm.get_runtimes().get("un1").unwrap().client_count,
        1,
        "共端口模式 WS 客户端应正常登记"
    );
    ws.send(Message::Text(
        serde_json::json!({ "event": "e", "data": {} }).to_string(),
    ))
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        sm.get_runtimes().get("un1").unwrap().received_messages >= 1,
        "共端口模式 WS 消息应正常接收"
    );

    sm.stop("un1").await.unwrap();
}

/// T6：独立 Mock 服务 customPort —— 启停端口正确、规则命中、停止后端口释放
#[tokio::test]
async fn mock_custom_port_listens_and_serves() {
    let dir = std::env::temp_dir().join(format!("ssm_mock_e2e_{}", uuid::Uuid::new_v4()));
    let _ = std::fs::create_dir_all(&dir);
    let cm = Arc::new(ConfigManager::new(dir));
    cm.init();
    let sys = SystemSettings::default();
    let mgr = Arc::new(MockManager::new());
    // ⚠️ 端口选择：
    // 1) 不用 free_port()（bind:0 立即释放）：Windows 上监听端口紧邻 rebind 可能 EADDRINUSE，
    //    且 bind_with_release 的 taskkill 有误杀测试进程自身的风险；
    // 2) 不用固定端口：服务端主动关闭连接后监听端口进入 TIME_WAIT（约 120s），
    //    残留会阻断后续测试的 bind。
    // 方案：基于进程 PID 选高位端口（31000 + PID%500，<49152 避开系统动态端口池），
    // 每次测试进程端口不同，天然避开 TIME_WAIT 残留。
    let port = 31000 + (std::process::id() % 500) as u32;

    let cfg = MockServiceConfig {
        id: "mc1".to_string(),
        name: "mock-e2e".to_string(),
        base_path: "/mock".to_string(),
        custom_port: Some(port as u16),
        rules: vec![MockRule {
            id: "mr1".to_string(),
            name: "hit".to_string(),
            method: HttpMethod::Get,
            // 独立端口模式：规则匹配剥离 base_path 后的相对路径
            path_pattern: "/hello".to_string(),
            response_status_code: 200,
            response_body: r#"{"hi":"there"}"#.to_string(),
            enabled: true,
            ..Default::default()
        }],
        // 显式默认响应（派生 Default 的 default_response_body 为空串；
        // 生产环境 config 反序列化时 serde default 才是 {"message":"ok"}）
        default_response_body: r#"{"message":"ok"}"#.to_string(),
        // add_service 在 enabled=true 时会自动启动（真实使用路径：mock_add 后即运行）
        enabled: true,
        ..Default::default()
    };
    mgr.add_service(cfg, &cm, &sys).await.expect("mock add");
    // 自动启动后：running_ids 登记 + 端口生效
    assert!(
        mgr.running_ids().contains(&"mc1".to_string()),
        "add 后应自动启动"
    );
    let started = mgr.port_of("mc1").expect("port_of") as u32;
    assert_eq!(started, port, "自定义端口应监听所分配端口");

    // 命中规则
    let (status, body) = http_req(started as u32, "GET", "/mock/hello", None).await;
    assert_eq!(status, 200);
    assert!(body.contains("hi"), "命中规则应返回规则响应体，body={body}");
    // 未命中 → MockServiceConfig 默认响应（default_status_code=200 + default_response_body={"message":"ok"}）
    let (s2, b2) = http_req(started as u32, "GET", "/mock/nope", None).await;
    assert_eq!(s2, 200, "未命中应返回默认状态码，body={b2}");
    assert!(
        b2.contains("message"),
        "未命中应返回默认响应体，body={b2}"
    );

    // 停止后：运行时句柄移除（Windows 上监听端口因 TIME_WAIT 无法即时 rebind，
    // 属正常 TCP 行为，不在此断言端口可立即重绑）
    mgr.stop_service("mc1").await;
    assert!(
        !mgr.running_ids().contains(&"mc1".to_string()),
        "停止后 running_ids 不应包含 mc1"
    );
}

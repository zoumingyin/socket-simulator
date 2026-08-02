//! 管理端 WebSocket 通道模块

pub mod admin;


#[cfg(test)]
mod smoke {
    use std::collections::HashSet;
    use std::time::Duration;
    use axum::Router;
    use futures_util::{SinkExt, StreamExt};
    use tokio::net::TcpListener;
    use tokio_tungstenite::tungstenite::Message;
    use crate::backend::app::Backend;
    use crate::backend::api::router::build_router;
    use crate::backend::constants::*;
    use crate::backend::types::WsFrame;

    #[tokio::test]
    async fn admin_ws_serves_initial_snapshots() {
        let dir = std::env::temp_dir().join(format!("ssm_ws_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp data dir");
        let backend = Backend::new_test(dir.clone());
        let state = std::sync::Arc::new(backend);
        let app: Router = build_router(state);
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind test port");
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move { let _ = axum::serve(listener, app).await; });
        let url = format!("ws://127.0.0.1:{port}{path}", port = port, path = ADMIN_WS_PATH);
        let (mut ws, _) = tokio_tungstenite::connect_async(url.as_str()).await.expect("connect to /admin/ws");
        let mut seen: HashSet<String> = HashSet::new();
        while seen.len() < 3 {
            let msg = tokio::time::timeout(Duration::from_secs(5), ws.next()).await
                .expect("ws read timed out").expect("ws stream ended").expect("ws message error");
            if let Message::Text(text) = msg {
                if let Ok(frame) = serde_json::from_str::<WsFrame>(&text) { seen.insert(frame.event.clone()); }
            }
        }
        assert!(seen.contains(EVT_RUNTIME_UPDATE), "missing runtime_update, got {:?}", seen);
        assert!(seen.contains(EVT_CLIENT_UPDATE), "missing client_update, got {:?}", seen);
        assert!(seen.contains(EVT_LOG_BATCH), "missing log_batch, got {:?}", seen);
        let ack = WsFrame { event: EVT_HEARTBEAT_ACK.to_string(), data: serde_json::json!({}) };
        ws.send(Message::Text(serde_json::to_string(&ack).unwrap())).await.expect("send heartbeat_ack");
        let _ = ws.close(None).await;
        let _ = std::fs::remove_dir_all(&dir);
    }
}

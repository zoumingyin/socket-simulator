//! ServiceManager —— 受管 Socket 服务生命周期管理（≡ Node ServiceManager）
//!
//! 负责：注册/启停受管服务（每个 ServerConfig 对应一个 `WsServer` 传输层）、维护运行时
//! 快照 `ServerRuntime`、把变更发布到 EventBus，并向客户端发送/广播消息。端口冲突时复用
//! `net::port_release` 释放占用进程（等价于 Node 的 killPort 自动重试）。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::backend::constants::*;
use crate::backend::error::BackendError;
use crate::backend::eventbus::EventBus;
use crate::backend::transport::hooks::TransportHooks;
use crate::backend::transport::{AdapterKind, AdapterRegistry, ProtocolAdapter};
use crate::backend::types::*;

use super::client_manager::ClientManager;
use super::config_manager::ConfigManager;
use super::log_manager::LogManager;

/// 服务管理器
pub struct ServiceManager {
    pub(crate) config: Arc<ConfigManager>,
    logs: Arc<LogManager>,
    clients: Arc<ClientManager>,
    event_bus: EventBus,
    /// 协议适配器注册表（P0-5：按 AdapterKind 创建传输实例，插件可注册/覆盖）
    registry: Arc<AdapterRegistry>,
    /// serverId -> 运行中的协议适配器实例（经注册表创建，可能为 WsServer / SocketIoServer / HttpServer / UnifiedServer）
    servers: Arc<Mutex<HashMap<String, Arc<dyn ProtocolAdapter>>>>,
    /// serverId -> 运行时快照
    runtimes: Arc<Mutex<HashMap<String, ServerRuntime>>>,
}

impl ServiceManager {
    pub fn new(
        config: Arc<ConfigManager>,
        logs: Arc<LogManager>,
        clients: Arc<ClientManager>,
        event_bus: EventBus,
    ) -> Self {
        Self {
            config,
            logs,
            clients,
            event_bus,
            registry: Arc::new(AdapterRegistry::new()),
            servers: Arc::new(Mutex::new(HashMap::new())),
            runtimes: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 注入自定义适配器注册表（P2-4 预留协议在此挂载；默认内置四类）
    /// 当前无调用方（new 内置默认注册表），预留注入点
    #[allow(dead_code)]
    pub fn with_registry(mut self, registry: Arc<AdapterRegistry>) -> Self {
        self.registry = registry;
        self
    }

    /// 用配置中的服务列表初始化运行时快照（全部 Stopped）
    pub fn reload(&self) {
        let servers = self.config.get_servers();
        let mut r = self.runtimes.lock().unwrap();
        r.clear();
        for s in servers {
            r.insert(
                s.id.clone(),
                ServerRuntime {
                    id: s.id.clone(),
                    status: ServerStatus::Stopped,
                    ..Default::default()
                },
            );
        }
    }

    /// 注册一个服务配置（建立运行时占位，状态 Stopped）
    pub fn register_server(&self, cfg: ServerConfig) {
        let mut r = self.runtimes.lock().unwrap();
        r.insert(
            cfg.id.clone(),
            ServerRuntime {
                id: cfg.id.clone(),
                status: ServerStatus::Stopped,
                ..Default::default()
            },
        );
    }

    /// 移除服务（运行中则拒绝）
    pub fn remove_server(&self, id: &str) -> bool {
        {
            let servers = self.servers.lock().unwrap();
            if servers.contains_key(id) {
                return false;
            }
        }
        self.runtimes.lock().unwrap().remove(id);
        true
    }

    /// 启动指定服务
    pub async fn start(&self, id: String) -> Result<(), BackendError> {
        {
            let servers = self.servers.lock().unwrap();
            if servers.contains_key(&id) {
                return Ok(());
            }
        }
        let cfg = self
            .config
            .get_server_by_id(&id)
            .ok_or(BackendError::ServerNotFound)?;
        let sys = self.config.get_system_settings();

        // 各 hook 闭包独立捕获所需共享引用
        let c1 = self.clients.clone();
        let eb1 = self.event_bus.clone();
        let lg1 = self.logs.clone();
        let rt1 = self.runtimes.clone();
        let sid1 = id.clone();

        let rt2 = self.runtimes.clone();
        let lg2 = self.logs.clone();
        let sid2 = id.clone();

        let c3 = self.clients.clone();
        let eb3 = self.event_bus.clone();
        let lg3 = self.logs.clone();
        let rt3 = self.runtimes.clone();
        let sid3 = id.clone();

        let hooks = TransportHooks {
            on_connect: Arc::new(move |info: ClientInfo| {
                c1.add(info.clone());
                eb1.publish_client(c1.list());
                {
                    let mut r = rt1.lock().unwrap();
                    if let Some(rt) = r.get_mut(&sid1) {
                        rt.client_count += 1;
                        rt.total_connections += 1;
                    }
                }
                eb1.publish_runtime(rt1.lock().unwrap().clone());
                lg1.add_entry(LogEntry {
                    server_id: Some(sid1.clone()),
                    level: LogLevel::Info,
                    event: "client_connect".to_string(),
                    message: format!("客户端已连接: {}", info.id),
                    client_id: Some(info.id.clone()),
                    ..Default::default()
                });
            }),
            on_message: Arc::new(move |socket_id: String, event: String, data: serde_json::Value| {
                {
                    let mut r = rt2.lock().unwrap();
                    if let Some(rt) = r.get_mut(&sid2) {
                        rt.received_messages += 1;
                    }
                }
                lg2.add_entry(LogEntry {
                    server_id: Some(sid2.clone()),
                    level: LogLevel::Info,
                    event: event.clone(),
                    message: format!("收到消息 [{}]: {}", socket_id, event),
                    client_id: Some(socket_id.clone()),
                    metadata: Some(data),
                    ..Default::default()
                });
            }),
            on_disconnect: Arc::new(move |socket_id: String| {
                c3.remove(&sid3, &socket_id);
                eb3.publish_client(c3.list());
                {
                    let mut r = rt3.lock().unwrap();
                    if let Some(rt) = r.get_mut(&sid3) {
                        rt.client_count = rt.client_count.saturating_sub(1);
                    }
                }
                eb3.publish_runtime(rt3.lock().unwrap().clone());
                lg3.add_entry(LogEntry {
                    server_id: Some(sid3.clone()),
                    level: LogLevel::Info,
                    event: "client_disconnect".to_string(),
                    message: format!("客户端已断开: {}", socket_id),
                    client_id: Some(socket_id.clone()),
                    ..Default::default()
                });
            }),
        };

        // P0-2: Socket.IO 与 Mock HTTP 共端口不兼容（hyper 1.0 / axum 0.7 与 socketioxide 统一路由冲突）。
        // 显式拒绝该组合，禁止静默进入 Unified（否则 Socket.IO 实际不会启动，协议静默丢失）。
        if cfg.mock_enabled && cfg.protocol == ProtocolType::SocketIo {
            return Err(BackendError::Config(
                "Socket.IO 协议不支持与 Mock HTTP 共端口（统一路由模式）。请关闭该服务的 Mock，或改用 WebSocket / HTTP 协议。".to_string(),
            ));
        }
        // P0-5: 经适配器注册表创建传输实例（内置：websocket / socketio / http / unified；
        // 插件可注册新协议并在此自动接管）。
        let kind = AdapterKind::from_cfg(&cfg);
        let adapter = self
            .registry
            .create(kind, cfg, sys, hooks)
            .ok_or_else(|| {
                BackendError::Config(format!("未注册的协议适配器: {}", kind.as_str()))
            })?;
        adapter.start().await?;
        self.servers.lock().unwrap().insert(id.clone(), adapter);
        {
            let mut r = self.runtimes.lock().unwrap();
            let rt = r
                .entry(id.clone())
                .or_insert_with(|| ServerRuntime {
                    id: id.clone(),
                    ..Default::default()
                });
            rt.status = ServerStatus::Running;
            rt.started_at = Some(now_rfc3339());
            rt.error = None;
            let snapshot = r.clone();
            self.event_bus.publish_runtime(snapshot);
        }
        Ok(())
    }

    /// 停止指定服务
    pub async fn stop(&self, id: &str) -> Result<(), BackendError> {
        let ws = self.servers.lock().unwrap().remove(id);
        if let Some(ws) = ws {
            ws.stop().await?;
        }
        {
            let mut r = self.runtimes.lock().unwrap();
            if let Some(rt) = r.get_mut(id) {
                rt.status = ServerStatus::Stopped;
                rt.stopped_at = Some(now_rfc3339());
            }
            self.event_bus.publish_runtime(r.clone());
        }
        Ok(())
    }

    /// 重启指定服务
    pub async fn restart(&self, id: String) -> Result<(), BackendError> {
        self.stop(&id).await?;
        tokio::time::sleep(Duration::from_millis(150)).await;
        self.start(id).await
    }

    /// 启动全部服务
    pub async fn start_all(&self) -> Result<(), BackendError> {
        for s in self.config.get_servers() {
            if let Err(e) = self.start(s.id.clone()).await {
                eprintln!("[ServiceManager] 启动服务 {} 失败: {}", s.id, e);
            }
        }
        Ok(())
    }

    /// 停止全部服务
    pub async fn stop_all(&self) -> Result<(), BackendError> {
        let ids: Vec<String> = self.servers.lock().unwrap().keys().cloned().collect();
        for id in ids {
            let _ = self.stop(&id).await;
        }
        Ok(())
    }

    /// 重启全部服务
    pub async fn restart_all(&self) -> Result<(), BackendError> {
        let ids: Vec<String> = self.config.get_servers().iter().map(|s| s.id.clone()).collect();
        for id in ids {
            let _ = self.restart(id).await;
        }
        Ok(())
    }

    /// 启动场景（P1-3）：按 `server_ids` 顺序逐个启动；单点失败记日志继续，返回逐服务结果。
    pub async fn start_scene(&self, scene: &SceneConfig) -> Vec<SceneServerResult> {
        let mut results = Vec::with_capacity(scene.server_ids.len());
        for sid in &scene.server_ids {
            match self.start(sid.clone()).await {
                Ok(()) => results.push(SceneServerResult {
                    server_id: sid.clone(),
                    success: true,
                    error: None,
                }),
                Err(e) => {
                    eprintln!(
                        "[ServiceManager] 场景 {} 启动服务 {} 失败: {}",
                        scene.name, sid, e
                    );
                    results.push(SceneServerResult {
                        server_id: sid.clone(),
                        success: false,
                        error: Some(e.to_string()),
                    });
                }
            }
        }
        results
    }

    /// 停止场景（P1-3）：逆序停止，返回成功停止数。
    pub async fn stop_scene(&self, scene: &SceneConfig) -> usize {
        let mut stopped = 0;
        for sid in scene.server_ids.iter().rev() {
            if self.stop(sid).await.is_ok() {
                stopped += 1;
            }
        }
        stopped
    }

    /// 全量运行时快照
    pub fn get_runtimes(&self) -> HashMap<String, ServerRuntime> {
        self.runtimes.lock().unwrap().clone()
    }

    /// 累加发送消息数并广播运行时
    pub fn increment_sent_messages(&self, id: &str) {
        {
            let mut r = self.runtimes.lock().unwrap();
            if let Some(rt) = r.get_mut(id) {
                rt.sent_messages += 1;
            }
        }
        self.event_bus.publish_runtime(self.runtimes.lock().unwrap().clone());
    }

    /// 广播消息给某个服务的全部在线客户端
    pub async fn broadcast(
        &self,
        server_id: &str,
        event: &str,
        data: serde_json::Value,
    ) -> Result<(), BackendError> {
        let server = self.servers.lock().unwrap().get(server_id).cloned();
        match server {
            Some(s) => s.broadcast(event, data, None).await,
            None => Err(BackendError::TransportNotFound),
        }
    }

    /// 发送消息并写日志（≡ Node sendMessageAndLog）
    /// target_id 可为复合键 `serverId___clientId`，此处自动提取原始 socketId
    pub async fn send_message(
        &self,
        server_id: &str,
        target_type: &str,
        target_id: Option<&str>,
        event: &str,
        data: serde_json::Value,
    ) -> Result<(), BackendError> {
        let server = self.servers.lock().unwrap().get(server_id).cloned();
        let Some(server) = server else {
            return Err(BackendError::TransportNotFound);
        };
        if target_type == "broadcast" || target_id.is_none() {
            server.broadcast(event, data.clone(), None).await?;
        } else {
            let actual = target_id.unwrap();
            let socket_id = if actual.contains(CLIENT_ID_SEP) {
                actual
                    .splitn(2, CLIENT_ID_SEP)
                    .nth(1)
                    .unwrap_or(actual)
            } else {
                actual
            };
            server.send(socket_id, event, data.clone()).await?;
        }
        self.increment_sent_messages(server_id);
        self.logs.add_entry(LogEntry {
            server_id: Some(server_id.to_string()),
            level: LogLevel::Info,
            event: event.to_string(),
            message: format!("[消息中心] 发送 → 事件: {}, 内容: {}", event, data),
            metadata: Some(serde_json::json!({ "targetType": target_type, "targetId": target_id })),
            ..Default::default()
        });
        Ok(())
    }

    /// 断开某个服务的指定客户端
    pub async fn disconnect_client(
        &self,
        server_id: &str,
        client_id: &str,
    ) -> Result<(), BackendError> {
        let server = self.servers.lock().unwrap().get(server_id).cloned();
        match server {
            Some(s) => s.disconnect_client(client_id).await,
            None => Err(BackendError::TransportNotFound),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::ServiceManager;
    use crate::backend::error::BackendError;
    use crate::backend::eventbus::EventBus;
    use crate::backend::managers::client_manager::ClientManager;
    use crate::backend::managers::config_manager::ConfigManager;
    use crate::backend::managers::log_manager::LogManager;
    use crate::backend::transport::hooks::TransportHooks;
    use crate::backend::transport::websocket::WsServer;
    use crate::backend::transport::ProtocolAdapter;
    use crate::backend::types::*;

    fn tmp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ssm_svc_{}", uuid::Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    fn setup() -> Arc<ServiceManager> {
        let dir = tmp_dir();
        let cm = Arc::new(ConfigManager::new(dir.clone()));
        let bus = EventBus::new();
        let log_m = Arc::new(LogManager::new(dir.join("logs"), bus.clone()));
        let client_m = Arc::new(ClientManager::new());
        Arc::new(ServiceManager::new(cm, log_m, client_m, bus))
    }

    fn noop_hooks() -> TransportHooks {
        TransportHooks {
            on_connect: Arc::new(|_info: ClientInfo| {}),
            on_message: Arc::new(|_sock: String, _ev: String, _data: serde_json::Value| {}),
            on_disconnect: Arc::new(|_sock: String| {}),
        }
    }

    #[test]
    fn register_server_creates_stopped_runtime() {
        let sm = setup();
        sm.register_server(ServerConfig {
            id: "s1".to_string(),
            ..Default::default()
        });
        let runtimes = sm.get_runtimes();
        assert_eq!(runtimes.len(), 1);
        let rt = runtimes.get("s1").expect("runtime exists");
        assert_eq!(rt.status, ServerStatus::Stopped);
    }

    #[test]
    fn remove_server_succeeds_when_not_running() {
        let sm = setup();
        sm.register_server(ServerConfig {
            id: "s1".to_string(),
            ..Default::default()
        });
        assert!(sm.remove_server("s1"));
        assert!(sm.get_runtimes().get("s1").is_none());
        assert!(sm.remove_server("s1"), "removing an absent server is a no-op success");
    }

    #[test]
    fn remove_server_refuses_when_running() {
        let sm = setup();
        sm.register_server(ServerConfig {
            id: "live".to_string(),
            ..Default::default()
        });
        // simulate a running transport by injecting a WsServer into the private map
        let ws: Arc<WsServer> = Arc::new_cyclic(|weak| {
            WsServer::new(
                ServerConfig {
                    id: "live".to_string(),
                    ..Default::default()
                },
                SystemSettings::default(),
                noop_hooks(),
                weak.clone(),
            )
        });
        sm.servers.lock().unwrap().insert("live".to_string(), ws as Arc<dyn ProtocolAdapter>);
        assert!(
            !sm.remove_server("live"),
            "remove_server must refuse while the transport is present"
        );
        // drop the fake transport explicitly (no live accept loop to leak)
        sm.servers.lock().unwrap().remove("live");
    }

    #[test]
    fn eventbus_delivers_runtime_updates() {
        let sm = setup();
        sm.register_server(ServerConfig {
            id: "s1".to_string(),
            ..Default::default()
        });
        // subscribe to the manager's own EventBus runtime channel
        let mut rx = sm.event_bus.runtime_tx.subscribe();
        // a ServiceManager method that publishes to the EventBus
        sm.increment_sent_messages("s1");
        let received = rx
            .try_recv()
            .expect("subscriber should receive the runtime snapshot");
        assert!(received.contains_key("s1"));
        assert_eq!(received.get("s1").unwrap().id, "s1");
    }

    #[test]
    fn broadcast_without_server_errors() {
        let sm = setup();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let res = rt.block_on(sm.broadcast("nope", "evt", serde_json::json!({"x": 1})));
        assert!(matches!(res, Err(BackendError::TransportNotFound)));
    }
}

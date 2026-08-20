//! v3 P0-5 协议适配器抽象（ProtocolAdapter + AdapterRegistry）
//!
//! 把「协议实现」抽象为可插拔适配器，服务生命周期与消息收发经注册表统一创建，
//! 替代 ServiceManager 内手写的 `match protocol` 分支：
//!
//! - `ProtocolAdapter`：完整协议适配器接口（协议元信息 + 继承 `Transport` 的生命周期/消息）
//! - `AdapterKind`：适配器标识（内置 websocket / socketio / http / unified + 预留扩展）
//! - `AdapterRegistry`：线程安全注册表，按 kind 创建适配器；插件可注册 / 覆盖
//!
//! 扩展点（P2-4 预留 TCP / UDP / MQTT / SSE）：新增协议只需实现 `ProtocolAdapter`，
//! 并在注册表 `register` 一个工厂，ServiceManager 零改动即可拉起。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::backend::transport::hooks::TransportHooks;
use crate::backend::transport::Transport;
use crate::backend::types::{ProtocolType, ServerConfig, SystemSettings};

/// 协议适配器：在 `Transport` 基础上附加协议元信息。
/// 实现方只需补齐元信息方法；生命周期与消息方法继承自 `Transport`。
#[async_trait]
pub trait ProtocolAdapter: Transport {
    /// 适配器对应的协议（unified 返回其底层协议）
    fn protocol(&self) -> ProtocolType;

    /// 所属服务 ID
    fn server_id(&self) -> &str;

    /// 是否为统一路由适配器（共端口 Mock + Socket）
    fn is_unified(&self) -> bool {
        false
    }
}

/// 适配器种类（注册表 key）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AdapterKind {
    Websocket,
    SocketIo,
    Http,
    /// 共端口模式（Mock HTTP + Socket 同一端口，对应 UnifiedServer）
    Unified,
    /// 预留协议（P2-4 骨架，尚未实现）
    Tcp,
    /// 预留协议（P2-4 骨架，尚未实现）
    Udp,
    /// 预留协议（P2-4 骨架，尚未实现）
    Mqtt,
    /// 预留协议（P2-4 骨架，尚未实现）
    Sse,
}

impl AdapterKind {
    /// 按服务配置选择适配器：启用 Mock 时走统一路由
    pub fn from_cfg(cfg: &ServerConfig) -> AdapterKind {
        if cfg.mock_enabled {
            AdapterKind::Unified
        } else {
            Self::from_protocol(cfg.protocol)
        }
    }

    pub fn from_protocol(p: ProtocolType) -> AdapterKind {
        match p {
            ProtocolType::Websocket => AdapterKind::Websocket,
            ProtocolType::SocketIo => AdapterKind::SocketIo,
            ProtocolType::Http => AdapterKind::Http,
            ProtocolType::Tcp => AdapterKind::Tcp,
            ProtocolType::Udp => AdapterKind::Udp,
            ProtocolType::Mqtt => AdapterKind::Mqtt,
            ProtocolType::Sse => AdapterKind::Sse,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            AdapterKind::Websocket => "websocket",
            AdapterKind::SocketIo => "socketio",
            AdapterKind::Http => "http",
            AdapterKind::Unified => "unified",
            AdapterKind::Tcp => "tcp",
            AdapterKind::Udp => "udp",
            AdapterKind::Mqtt => "mqtt",
            AdapterKind::Sse => "sse",
        }
    }
}

/// 适配器工厂：由配置 + 系统设置 + hooks 构造适配器实例
pub type AdapterFactoryFn = Arc<
    dyn Fn(ServerConfig, SystemSettings, TransportHooks) -> Arc<dyn ProtocolAdapter> + Send + Sync,
>;

/// 适配器注册表（线程安全；支持插件注册 / 覆盖）
pub struct AdapterRegistry {
    factories: Mutex<HashMap<AdapterKind, AdapterFactoryFn>>,
}

impl AdapterRegistry {
    /// 内置注册：websocket / socketio / http / unified（与既有传输实现一一对应）
    pub fn new() -> Self {
        let reg = Self {
            factories: Mutex::new(HashMap::new()),
        };
        reg.register_builtin();
        reg
    }

    fn register_builtin(&self) {
        use crate::backend::transport::http::HttpServer;
        use crate::backend::transport::mqtt::MqttAdapter;
        use crate::backend::transport::sse::SseAdapter;
        use crate::backend::transport::socketio::SocketIoServer;
        use crate::backend::transport::tcp::TcpAdapter;
        use crate::backend::transport::udp::UdpAdapter;
        use crate::backend::transport::unified::UnifiedServer;
        use crate::backend::transport::websocket::WsServer;

        self.register(AdapterKind::Websocket, Arc::new(|cfg, sys, hooks| {
            // WsServer 通过 new_cyclic 注入 self 弱引用（accept loop 取 Arc<Self>）
            let ws: Arc<WsServer> =
                Arc::new_cyclic(|weak| WsServer::new(cfg, sys, hooks, weak.clone()));
            ws as Arc<dyn ProtocolAdapter>
        }));
        self.register(AdapterKind::SocketIo, Arc::new(|cfg, sys, hooks| {
            let sio = Arc::new(SocketIoServer::new(cfg, sys, hooks));
            sio as Arc<dyn ProtocolAdapter>
        }));
        self.register(AdapterKind::Http, Arc::new(|cfg, sys, hooks| {
            let http = Arc::new(HttpServer::new(cfg, sys, hooks));
            http as Arc<dyn ProtocolAdapter>
        }));
        self.register(AdapterKind::Unified, Arc::new(|cfg, sys, hooks| {
            let unified = Arc::new(UnifiedServer::new(cfg, sys, hooks));
            unified as Arc<dyn ProtocolAdapter>
        }));
        // 预留协议（P2-4 骨架）：注册工厂，start() 返回 NotImplemented，尚未实现
        self.register(AdapterKind::Tcp, Arc::new(|cfg, sys, hooks| {
            TcpAdapter::new(cfg, sys, hooks) as Arc<dyn ProtocolAdapter>
        }));
        self.register(AdapterKind::Udp, Arc::new(|cfg, sys, hooks| {
            UdpAdapter::new(cfg, sys, hooks) as Arc<dyn ProtocolAdapter>
        }));
        self.register(AdapterKind::Mqtt, Arc::new(|cfg, sys, hooks| {
            MqttAdapter::new(cfg, sys, hooks) as Arc<dyn ProtocolAdapter>
        }));
        self.register(AdapterKind::Sse, Arc::new(|cfg, sys, hooks| {
            SseAdapter::new(cfg, sys, hooks) as Arc<dyn ProtocolAdapter>
        }));
    }

    /// 注册 / 覆盖适配器工厂（P2-4 预留协议在此注册）
    pub fn register(&self, kind: AdapterKind, factory: AdapterFactoryFn) {
        self.factories.lock().unwrap().insert(kind, factory);
    }

    /// 按 kind 创建适配器实例；未注册返回 None
    pub fn create(
        &self,
        kind: AdapterKind,
        cfg: ServerConfig,
        sys: SystemSettings,
        hooks: TransportHooks,
    ) -> Option<Arc<dyn ProtocolAdapter>> {
        let f = self.factories.lock().unwrap().get(&kind).cloned()?;
        Some(f(cfg, sys, hooks))
    }

    /// 已注册种类（调试 / 审计用）
    pub fn kinds(&self) -> Vec<AdapterKind> {
        self.factories.lock().unwrap().keys().copied().collect()
    }
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::error::BackendError;
    use crate::backend::types::ClientInfo;

    fn noop_hooks() -> TransportHooks {
        TransportHooks {
            on_connect: Arc::new(|_info: ClientInfo| {}),
            on_message: Arc::new(|_sock: String, _ev: String, _data: serde_json::Value| {}),
            on_disconnect: Arc::new(|_sock: String| {}),
        }
    }

    fn sample_cfg(protocol: ProtocolType) -> ServerConfig {
        ServerConfig {
            id: "s1".to_string(),
            protocol,
            ..Default::default()
        }
    }

    #[test]
    fn builtin_kinds_are_registered() {
        let reg = AdapterRegistry::new();
        assert_eq!(reg.kinds().len(), 8);
        for k in [
            AdapterKind::Websocket,
            AdapterKind::SocketIo,
            AdapterKind::Http,
            AdapterKind::Unified,
            AdapterKind::Tcp,
            AdapterKind::Udp,
            AdapterKind::Mqtt,
            AdapterKind::Sse,
        ] {
            assert!(reg.kinds().contains(&k), "{} 应已注册", k.as_str());
        }
    }

    #[test]
    fn reserved_kinds_are_registered() {
        let reg = AdapterRegistry::new();
        let cases = [
            (AdapterKind::Tcp, ProtocolType::Tcp),
            (AdapterKind::Udp, ProtocolType::Udp),
            (AdapterKind::Mqtt, ProtocolType::Mqtt),
            (AdapterKind::Sse, ProtocolType::Sse),
        ];
        for (kind, proto) in cases {
            assert!(reg.kinds().contains(&kind), "{} 应已注册", kind.as_str());
            let a = reg
                .create(kind, sample_cfg(proto), SystemSettings::default(), noop_hooks())
                .expect("reserved adapter 应可创建");
            assert_eq!(a.protocol(), proto);
            assert_eq!(a.server_id(), "s1");
            assert!(!a.is_unified());
        }
    }

    #[test]
    fn reserved_adapter_start_returns_not_implemented() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let reg = AdapterRegistry::new();
            let a = reg
                .create(
                    AdapterKind::Tcp,
                    sample_cfg(ProtocolType::Tcp),
                    SystemSettings::default(),
                    noop_hooks(),
                )
                .expect("tcp adapter 应可创建");
            let err = a
                .start()
                .await
                .expect_err("reserved adapter start 应返回 NotImplemented");
            assert!(matches!(err, BackendError::NotImplemented(_)));
        });
    }

    #[test]
    fn kind_from_cfg_selects_unified_when_mock_enabled() {
        let mut cfg = sample_cfg(ProtocolType::Websocket);
        cfg.mock_enabled = true;
        assert_eq!(AdapterKind::from_cfg(&cfg), AdapterKind::Unified);
        cfg.mock_enabled = false;
        assert_eq!(AdapterKind::from_cfg(&cfg), AdapterKind::Websocket);
    }

    #[test]
    fn create_builds_http_adapter_with_metadata() {
        let reg = AdapterRegistry::new();
        let cfg = sample_cfg(ProtocolType::Http);
        let a = reg
            .create(AdapterKind::Http, cfg, SystemSettings::default(), noop_hooks())
            .expect("http adapter 应可创建");
        assert_eq!(a.protocol(), ProtocolType::Http);
        assert_eq!(a.server_id(), "s1");
        assert!(!a.is_unified());
    }

    #[test]
    fn unified_adapter_reports_unified_flag() {
        let reg = AdapterRegistry::new();
        let mut cfg = sample_cfg(ProtocolType::Websocket);
        cfg.mock_enabled = true;
        let a = reg
            .create(AdapterKind::Unified, cfg, SystemSettings::default(), noop_hooks())
            .expect("unified adapter 应可创建");
        assert!(a.is_unified());
        assert_eq!(a.protocol(), ProtocolType::Websocket);
        assert_eq!(a.server_id(), "s1");
    }

    #[test]
    fn register_overrides_factory() {
        let reg = AdapterRegistry::new();
        // 覆盖 Http 工厂：返回一个带自定义 id 的 WsServer（仅验证覆盖机制生效）
        reg.register(
            AdapterKind::Http,
            Arc::new(|cfg, _sys, _hooks| {
                let ws: Arc<crate::backend::transport::websocket::WsServer> =
                    Arc::new_cyclic(|weak| {
                        crate::backend::transport::websocket::WsServer::new(
                            ServerConfig {
                                id: "overridden".to_string(),
                                ..cfg.clone()
                            },
                            SystemSettings::default(),
                            noop_hooks(),
                            weak.clone(),
                        )
                    });
                ws as Arc<dyn ProtocolAdapter>
            }),
        );
        let cfg = sample_cfg(ProtocolType::Http);
        let a = reg
            .create(AdapterKind::Http, cfg, SystemSettings::default(), noop_hooks())
            .expect("覆盖后的工厂应生效");
        assert_eq!(a.server_id(), "overridden");
    }
}

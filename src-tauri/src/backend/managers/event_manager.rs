//! EventManager —— 事件管理 + 定时轮询推送（≡ Node EventManager）
//!
//! 事件配置持久化在 `ConfigManager`（单一 config.json）。本管理器负责事件的增删改查，
//! 并在 `start` 后启动一个 ticker：对启用了轮询（polling_enabled）且到达间隔的事件，
//! 通过 `ServiceManager` 把 `defaultMessage`/`handler` 推送给对应服务的在线客户端。

use std::collections::HashMap;
use std::sync::Arc;

use tokio::time::{Duration, Instant};

use crate::backend::types::*;

use super::config_manager::ConfigManager;
use super::service_manager::ServiceManager;

/// 事件管理器
pub struct EventManager {
    config: Arc<ConfigManager>,
    services: Arc<ServiceManager>,
}

impl EventManager {
    pub fn new(config: Arc<ConfigManager>, services: Arc<ServiceManager>) -> Self {
        Self { config, services }
    }

    /// 新增事件（补全 id / 时间戳）
    pub fn add_event(&self, mut evt: EventConfig) -> EventConfig {
        if evt.id.is_empty() {
            evt.id = uuid::Uuid::new_v4().to_string();
        }
        if evt.created_at.is_empty() {
            evt.created_at = now_rfc3339();
        }
        if evt.updated_at.is_empty() {
            evt.updated_at = now_rfc3339();
        }
        let mut events = self.config.get_events();
        events.push(evt.clone());
        self.config.save_events(events);
        evt
    }

    /// 更新事件
    pub fn update_event(&self, id: &str, mut evt: EventConfig) -> Option<EventConfig> {
        let mut events = self.config.get_events();
        let idx = events.iter().position(|e| e.id == id)?;
        evt.id = id.to_string();
        evt.updated_at = now_rfc3339();
        events[idx] = evt.clone();
        self.config.save_events(events);
        Some(evt)
    }

    /// 删除事件
    pub fn remove_event(&self, id: &str) -> bool {
        let mut events = self.config.get_events();
        let before = events.len();
        events.retain(|e| e.id != id);
        let removed = events.len() < before;
        if removed {
            self.config.save_events(events);
        }
        removed
    }

    /// 切换事件启用状态
    pub fn toggle_event(&self, id: &str, status: EventStatus) -> Option<EventConfig> {
        let mut events = self.config.get_events();
        let idx = events.iter().position(|e| e.id == id)?;
        events[idx].status = status;
        events[idx].updated_at = now_rfc3339();
        let evt = events[idx].clone();
        self.config.save_events(events);
        Some(evt)
    }

    /// 导入事件（配置已由 ConfigManager 更新，此处为兼容 Node 调用占位）
    pub fn load_events(&self, _events: Vec<EventConfig>) {}

    /// 启动事件轮询：每秒检查一次，命中间隔即推送
    pub fn start(&self) {
        let config = self.config.clone();
        let services = self.services.clone();
        tauri::async_runtime::spawn(async move {
            let mut last: HashMap<String, Instant> = HashMap::new();
            let mut ticker = tokio::time::interval(Duration::from_secs(1));
            loop {
                ticker.tick().await;
                let events = config.get_events();
                let now = Instant::now();
                for e in events {
                    if e.status != EventStatus::Enabled {
                        continue;
                    }
                    if !e.polling_enabled {
                        continue;
                    }
                    let interval = e.polling_interval.unwrap_or(0);
                    if interval == 0 {
                        continue;
                    }
                    let prev = last.get(&e.id).copied().unwrap_or(now);
                    if now.duration_since(prev) >= Duration::from_millis(interval) {
                        last.insert(e.id.clone(), now);
                        let data = e
                            .default_message
                            .clone()
                            .map(|m| serde_json::json!(m))
                            .unwrap_or(serde_json::Value::Null);
                        let event_name = e.handler.clone().unwrap_or_else(|| e.name.clone());
                        let _ = services.broadcast(&e.server_id, &event_name, data).await;
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::EventManager;
    use crate::backend::eventbus::EventBus;
    use crate::backend::managers::client_manager::ClientManager;
    use crate::backend::managers::config_manager::ConfigManager;
    use crate::backend::managers::log_manager::LogManager;
    use crate::backend::managers::service_manager::ServiceManager;
    use crate::backend::types::*;

    fn setup() -> EventManager {
        let dir = std::env::temp_dir().join(format!("ssm_evt_{}", uuid::Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&dir);
        let cm = Arc::new(ConfigManager::new(dir.clone()));
        let bus = EventBus::new();
        let log_m = Arc::new(LogManager::new(dir.join("logs"), bus.clone()));
        let client_m = Arc::new(ClientManager::new());
        let sm = Arc::new(ServiceManager::new(
            cm.clone(),
            log_m.clone(),
            client_m.clone(),
            bus.clone(),
        ));
        EventManager::new(cm, sm)
    }

    fn mk_event(server_id: &str, name: &str) -> EventConfig {
        EventConfig {
            server_id: server_id.to_string(),
            name: name.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn add_event_autofills_id_and_timestamps() {
        let em = setup();
        let evt = em.add_event(mk_event("s1", "ev1"));
        assert!(!evt.id.is_empty(), "add_event should auto-generate id");
        assert!(!evt.created_at.is_empty());
        assert!(!evt.updated_at.is_empty());
        assert_eq!(evt.status, EventStatus::Enabled);
        assert_eq!(em.config.get_events().len(), 1);
    }

    /// 回归：/api/events/add 的 JSON 请求体不含 `id`（前端 addEvent 用
    /// Omit<EventConfig,'id'|'createdAt'|'updatedAt'> 发送），必须能反序列化成功，
    /// 反序列化后 id 为空，交由 add_event 自动补全。
    #[test]
    fn event_config_deserializes_without_id() {
        let body = r#"{
            "serverId": "9-nYx6OYZdEFGUAnL5-Ny",
            "name": "qw",
            "status": "enabled",
            "pollingEnabled": false,
            "pollingInterval": 10
        }"#;
        let parsed: EventConfig = serde_json::from_str(body)
            .expect("EventConfig 应能在缺少 id 时反序列化（回归：missing field `id` 错误）");
        assert!(parsed.id.is_empty(), "omit id 后应留空，由 add_event 补全");
        assert_eq!(parsed.server_id, "9-nYx6OYZdEFGUAnL5-Ny");
        assert_eq!(parsed.name, "qw");
        assert_eq!(parsed.status, EventStatus::Enabled);
        assert!(!parsed.polling_enabled);
        assert_eq!(parsed.polling_interval, Some(10));

        // 走真实 add_event 路径，确认最终被补全并落盘
        let em = setup();
        let saved = em.add_event(parsed);
        assert!(!saved.id.is_empty(), "add_event 必须补全 id");
        assert_eq!(em.config.get_events().len(), 1);
    }

    #[test]
    fn toggle_event_flips_enabled_flag() {
        let em = setup();
        let evt = em.add_event(mk_event("s1", "ev1"));
        let disabled = em
            .toggle_event(&evt.id, EventStatus::Disabled)
            .expect("event exists");
        assert_eq!(disabled.status, EventStatus::Disabled);
        assert_eq!(em.config.get_events()[0].status, EventStatus::Disabled);
        let enabled = em
            .toggle_event(&evt.id, EventStatus::Enabled)
            .expect("event exists");
        assert_eq!(enabled.status, EventStatus::Enabled);
        assert_eq!(em.config.get_events()[0].status, EventStatus::Enabled);
    }

    #[test]
    fn toggle_missing_event_returns_none() {
        let em = setup();
        assert!(em.toggle_event("nope", EventStatus::Disabled).is_none());
    }

    #[test]
    fn update_event_replaces_fields_keeps_id() {
        let em = setup();
        let evt = em.add_event(mk_event("s1", "ev1"));
        let mut updated = mk_event("s2", "ev2");
        updated.server_id = "s2".to_string();
        let result = em.update_event(&evt.id, updated).expect("event exists");
        assert_eq!(result.id, evt.id, "id must be preserved");
        assert_eq!(result.name, "ev2");
        assert_eq!(result.server_id, "s2");
        let stored = em.config.get_events();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].name, "ev2");
        assert_eq!(stored[0].id, evt.id);
    }

    #[test]
    fn remove_event_roundtrip() {
        let em = setup();
        let evt = em.add_event(mk_event("s1", "ev1"));
        assert!(em.remove_event(&evt.id));
        assert!(em.config.get_events().is_empty());
        assert!(!em.remove_event(&evt.id), "removing again returns false");
    }
}

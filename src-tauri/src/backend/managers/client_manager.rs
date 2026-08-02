//! ClientManager —— 客户端连接管理（≡ Node ClientManager）
//!
//! 以复合键 `serverId___socketId` 维护在线客户端，支持按服务过滤与全量查询。
//! 变更会通过 EventBus 推送给管理端 WS（由 ServiceManager 的 hook 触发）。

use std::collections::HashMap;
use std::sync::Mutex;

use crate::backend::constants::CLIENT_ID_SEP;
use crate::backend::types::*;

/// 客户端连接管理器
pub struct ClientManager {
    clients: Mutex<HashMap<String, ClientInfo>>,
}

impl ClientManager {
    pub fn new() -> Self {
        Self {
            clients: Mutex::new(HashMap::new()),
        }
    }

    fn composite(server_id: &str, socket_id: &str) -> String {
        format!("{}{}{}", server_id, CLIENT_ID_SEP, socket_id)
    }

    /// 新增/更新一个客户端连接
    pub fn add(&self, info: ClientInfo) {
        let key = Self::composite(&info.server_id, &info.id);
        self.clients.lock().unwrap().insert(key, info);
    }

    /// 移除指定服务的某个客户端（socket_id 为原始 id，不含 serverId 前缀）
    pub fn remove(&self, server_id: &str, socket_id: &str) {
        let key = Self::composite(server_id, socket_id);
        self.clients.lock().unwrap().remove(&key);
    }

    /// 全量客户端列表
    pub fn list(&self) -> Vec<ClientInfo> {
        self.clients.lock().unwrap().values().cloned().collect()
    }

    /// 按服务过滤（server_id 为 None 时返回全量）
    pub fn get_clients(&self, server_id: Option<&str>) -> Vec<ClientInfo> {
        let g = self.clients.lock().unwrap();
        match server_id {
            Some(sid) => g.values().filter(|c| c.server_id == sid).cloned().collect(),
            None => g.values().cloned().collect(),
        }
    }

    /// 指定服务的在线数量
    pub fn count(&self, server_id: &str) -> usize {
        self.clients
            .lock()
            .unwrap()
            .values()
            .filter(|c| c.server_id == server_id)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::constants::CLIENT_ID_SEP;
    use crate::backend::types::*;

    fn make_client(server_id: &str, id: &str) -> ClientInfo {
        ClientInfo {
            id: id.to_string(),
            server_id: server_id.to_string(),
            socket_id: id.to_string(),
            ip_address: "127.0.0.1".to_string(),
            status: ClientStatus::Connected,
            ..Default::default()
        }
    }

    #[test]
    fn add_then_list_and_count() {
        let m = ClientManager::new();
        m.add(make_client("s1", "c1"));
        let list = m.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "c1");
        assert_eq!(list[0].server_id, "s1");
        assert_eq!(m.count("s1"), 1);
    }

    #[test]
    fn composite_key_separates_servers() {
        let m = ClientManager::new();
        m.add(make_client("s1", "shared"));
        m.add(make_client("s2", "shared"));
        assert_eq!(m.list().len(), 2, "two servers keep distinct composite keys");
        assert_eq!(m.count("s1"), 1);
        assert_eq!(m.count("s2"), 1);
        // composite key separator is the documented "___"
        assert_eq!(CLIENT_ID_SEP, "___");
        let s1 = m.get_clients(Some("s1"));
        assert_eq!(s1.len(), 1);
        assert_eq!(s1[0].server_id, "s1");
    }

    #[test]
    fn duplicate_add_overwrites_same_key() {
        let m = ClientManager::new();
        m.add(make_client("s1", "a"));
        m.add(make_client("s1", "a"));
        assert_eq!(m.list().len(), 1, "same serverId___id overwrites one entry");
        assert_eq!(m.count("s1"), 1);
    }

    #[test]
    fn get_clients_filter_vs_none() {
        let m = ClientManager::new();
        m.add(make_client("s1", "a"));
        m.add(make_client("s2", "b"));
        assert_eq!(m.get_clients(None).len(), 2, "None returns all");
        assert_eq!(m.get_clients(Some("s1")).len(), 1);
        assert_eq!(m.get_clients(Some("nope")).len(), 0);
    }

    #[test]
    fn remove_drops_client() {
        let m = ClientManager::new();
        m.add(make_client("s1", "c1"));
        m.remove("s1", "c1");
        assert_eq!(m.list().len(), 0);
        assert_eq!(m.count("s1"), 0);
        m.remove("s1", "c1");
        assert_eq!(m.list().len(), 0, "removing again is a no-op");
    }
}

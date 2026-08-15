//! manager.rs —— Mock 服务生命周期管理 + 主端口分发
//!
//! - 自定义端口：每个服务独立 TcpListener，shutdown 用 oneshot signal
//! - 主端口（custom_port=None）：由 AppState 提供 dispatch_main_port，主 axum 路由的 fallback 调用
//!
//! 不持久化配置（ConfigManager 已持久化），仅持有运行时句柄。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::extract::Request;
use axum::response::Response;
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::backend::constants::api_port;
use crate::backend::managers::config_manager::ConfigManager;
use crate::backend::types::{MockServiceConfig, SystemSettings, now_rfc3339};

use super::server::{dispatch, MockServer};

#[derive(Default)]
struct RunningHandle {
    /// 自定义端口的关闭 signal
    shutdown: Option<oneshot::Sender<()>>,
    /// 实际绑定端口
    port: u16,
}

/// Mock 服务 CRUD/启停门面错误
///
/// 携带与 HTTP 响应对应的 `code` / `status` / `message`，便于 handler 层
/// 直接映射为 `ApiResponse`，避免在 handler 中重复散布校验逻辑。
#[derive(Debug)]
pub struct MockFacadeError {
    pub code: &'static str,
    pub status: u16,
    pub message: String,
}

/// Mock 服务管理器
pub struct MockManager {
    handles: Mutex<HashMap<String, RunningHandle>>,
}

impl MockManager {
    pub fn new() -> Self {
        Self {
            handles: Mutex::new(HashMap::new()),
        }
    }

    /// 启动一个 Mock 服务：主端口（custom_port=None）只标记；自定义端口启动独立 listener
    pub async fn start(
        &self,
        cfg: MockServiceConfig,
        sys: &SystemSettings,
    ) -> Result<u16, String> {
        if !cfg.enabled {
            return Err("服务未启用".to_string());
        }
        self.stop(&cfg.id).await;

        let id = cfg.id.clone();
        match cfg.custom_port {
            None => {
                let mut g = self.handles.lock().unwrap();
                g.insert(
                    id,
                    RunningHandle {
                        shutdown: None,
                        port: 0,
                    },
                );
                Ok(0)
            }
            Some(port) => {
                let tx = MockServer::start_custom_port(cfg, sys.clone(), port).await?;
                let mut g = self.handles.lock().unwrap();
                g.insert(
                    id,
                    RunningHandle {
                        shutdown: Some(tx),
                        port,
                    },
                );
                Ok(port)
            }
        }
    }

    /// 停止一个 Mock 服务
    pub async fn stop(&self, id: &str) {
        if let Some(mut h) = self.handles.lock().unwrap().remove(id) {
            if let Some(tx) = h.shutdown.take() {
                let _ = tx.send(());
            }
        }
    }

    // ==================== CRUD / 启停门面 ====================
    //
    // 把 handlers 中的「校验 + 持久化 + 启停」编排收敛到此，handler 仅做请求解析与响应映射。
    // 这些方法只依赖 `ConfigManager` + `SystemSettings`，不触碰任何 axum 内部状态，
    // 因此可脱离完整 axum 栈单测。

    /// 新增 Mock 服务：校验 basePath/端口 → 生成 id/时间戳 → 持久化 → 按需启动。
    pub async fn add_service(
        &self,
        mut cfg: MockServiceConfig,
        cfg_mgr: &ConfigManager,
        sys: &SystemSettings,
    ) -> Result<MockServiceConfig, MockFacadeError> {
        if cfg.id.is_empty() {
            cfg.id = Uuid::new_v4().to_string();
        }
        if cfg.base_path.starts_with("/admin/api") {
            return Err(MockFacadeError {
                code: "MOCK_BAD_BASEPATH",
                status: 400,
                message: "basePath 不能以 /admin/api 开头".into(),
            });
        }
        if let Some(p) = cfg.custom_port {
            if p == api_port() {
                return Err(MockFacadeError {
                    code: "MOCK_BAD_PORT",
                    status: 400,
                    message: "自定义端口不能与管理窗口端口相同".into(),
                });
            }
        }
        let now = now_rfc3339();
        if cfg.created_at.is_empty() {
            cfg.created_at = now.clone();
        }
        cfg.updated_at = now;

        let mut list = cfg_mgr.get_mock_services();
        list.push(cfg.clone());
        cfg_mgr.save_mock_services(list);

        if cfg.enabled {
            if let Err(e) = self.start(cfg.clone(), sys).await {
                return Err(MockFacadeError {
                    code: "MOCK_START_FAIL",
                    status: 500,
                    message: e,
                });
            }
        }
        Ok(cfg)
    }

    /// 更新 Mock 服务：校验 basePath/端口 → 刷新时间戳 → 持久化 → 重启（先停后启）。
    pub async fn update_service(
        &self,
        mut cfg: MockServiceConfig,
        cfg_mgr: &ConfigManager,
        sys: &SystemSettings,
    ) -> Result<MockServiceConfig, MockFacadeError> {
        if cfg.base_path.starts_with("/admin/api") {
            return Err(MockFacadeError {
                code: "MOCK_BAD_BASEPATH",
                status: 400,
                message: "basePath 不能以 /admin/api 开头".into(),
            });
        }
        if let Some(p) = cfg.custom_port {
            if p == api_port() {
                return Err(MockFacadeError {
                    code: "MOCK_BAD_PORT",
                    status: 400,
                    message: "自定义端口不能与管理窗口端口相同".into(),
                });
            }
        }
        cfg.updated_at = now_rfc3339();

        let mut list = cfg_mgr.get_mock_services();
        let mut found = false;
        for s in list.iter_mut() {
            if s.id == cfg.id {
                *s = cfg.clone();
                found = true;
                break;
            }
        }
        if !found {
            list.push(cfg.clone());
        }
        cfg_mgr.save_mock_services(list);

        self.stop(&cfg.id).await;
        if cfg.enabled {
            if let Err(e) = self.start(cfg.clone(), sys).await {
                return Err(MockFacadeError {
                    code: "MOCK_START_FAIL",
                    status: 500,
                    message: e,
                });
            }
        }
        Ok(cfg)
    }

    /// 删除 Mock 服务：先停后删配置。
    pub async fn remove_service(&self, id: &str, cfg_mgr: &ConfigManager) {
        self.stop(id).await;
        let list: Vec<MockServiceConfig> = cfg_mgr
            .get_mock_services()
            .into_iter()
            .filter(|s| s.id != id)
            .collect();
        cfg_mgr.save_mock_services(list);
    }

    /// 按 id 启动已存在的 Mock 服务（自定义端口立即监听，主端口仅标记）。
    pub async fn start_service(
        &self,
        id: &str,
        cfg_mgr: &ConfigManager,
        sys: &SystemSettings,
    ) -> Result<u16, MockFacadeError> {
        let cfg = match cfg_mgr.get_mock_service_by_id(id) {
            Some(s) => s,
            None => {
                return Err(MockFacadeError {
                    code: "MOCK_NOT_FOUND",
                    status: 404,
                    message: "mock 服务不存在".into(),
                })
            }
        };
        self.start(cfg, sys).await.map_err(|e| MockFacadeError {
            code: "MOCK_START_FAIL",
            status: 500,
            message: e,
        })
    }

    /// 停止指定 Mock 服务（门面版，供 handler 直接调用）。
    pub async fn stop_service(&self, id: &str) {
        self.stop(id).await;
    }

    /// 主端口分发：查找 basePath 匹配的服务，转交 dispatch；未匹配返回 None
    ///
    /// 返回 `Option<Response>`：
    /// - `Some(resp)` — 匹配到 Mock 服务，resp 为该服务的响应（可能是任意状态码）
    /// - `None` — 没有任何运行中的主端口 Mock 服务匹配该路径，调用方应继续 fallback（如前端静态文件）
    pub async fn dispatch_main_port(
        &self,
        cfg_mgr: &ConfigManager,
        sys: &SystemSettings,
        req: Request,
    ) -> Option<Response> {
        let full_path = req.uri().path().to_string();

        let running_ids: Vec<String> = self.handles.lock().unwrap().keys().cloned().collect();
        let mut best: Option<(MockServiceConfig, usize)> = None;
        for id in running_ids {
            if let Some(cfg) = cfg_mgr.get_mock_service_by_id(&id) {
                if cfg.custom_port.is_some() {
                    continue;
                }
                let base = cfg.base_path.trim_end_matches('/');
                if base.is_empty() {
                    continue;
                }
                if base.starts_with("/admin/api") {
                    continue;
                }
                if full_path == base || full_path.starts_with(&format!("{}/", base)) {
                    let score = base.len();
                    if best.as_ref().map(|(_, s)| score > *s).unwrap_or(true) {
                        best = Some((cfg, score));
                    }
                }
            }
        }

        match best {
            Some((cfg, _)) => Some(dispatch(&cfg, sys, req).await),
            None => None,
        }
    }

    /// 查询实际绑定端口
    pub fn port_of(&self, id: &str) -> Option<u16> {
        self.handles.lock().unwrap().get(id).map(|h| h.port)
    }

    /// 应用启动时恢复：custom_port 的立即启动；主端口的标记
    pub async fn restore(&self, cfg_mgr: &ConfigManager, sys: &SystemSettings) {
        for svc in cfg_mgr.get_mock_services() {
            if svc.enabled {
                if let Err(e) = self.start(svc, sys).await {
                    eprintln!("[MockManager] 恢复 mock 服务失败: {}", e);
                }
            }
        }
    }

    /// 全部停止
    pub async fn stop_all(&self) {
        let ids: Vec<String> = self.handles.lock().unwrap().keys().cloned().collect();
        for id in ids {
            self.stop(&id).await;
        }
    }

    /// 当前运行中的服务 ID 列表
    pub fn running_ids(&self) -> Vec<String> {
        self.handles.lock().unwrap().keys().cloned().collect()
    }
}

pub type SharedMockManager = Arc<MockManager>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::managers::config_manager::ConfigManager;
    use crate::backend::types::MockServiceConfig;
    use std::sync::Arc;

    fn temp_cfg() -> Arc<ConfigManager> {
        let dir =
            std::env::temp_dir().join(format!("ssm_mock_facade_{}", Uuid::new_v4()));
        let cm = Arc::new(ConfigManager::new(dir));
        cm.init();
        cm
    }

    #[tokio::test]
    async fn add_then_start_stop_service() {
        let mgr = MockManager::new();
        let cm = temp_cfg();
        let sys = SystemSettings::default();
        let cfg = MockServiceConfig {
            id: "m1".to_string(),
            name: "t".to_string(),
            enabled: true,
            ..Default::default()
        };
        // 主端口（无 custom_port）：add 成功并持久化；start 返回 0 且不真正监听
        let saved = mgr.add_service(cfg, &cm, &sys).await.unwrap();
        assert_eq!(saved.id, "m1");
        assert!(!saved.created_at.is_empty());
        assert!(!saved.updated_at.is_empty());
        // 配置已持久化
        assert!(cm.get_mock_service_by_id("m1").is_some());
        // 停止后不应再处于运行中
        mgr.stop_service("m1").await;
        assert!(!mgr.running_ids().contains(&"m1".to_string()));
    }

    #[tokio::test]
    async fn add_rejects_bad_basepath_and_port() {
        let mgr = MockManager::new();
        let cm = temp_cfg();
        let sys = SystemSettings::default();

        let bad_base = MockServiceConfig {
            id: "m2".to_string(),
            base_path: "/admin/api/x".to_string(),
            enabled: true,
            ..Default::default()
        };
        let e = mgr.add_service(bad_base, &cm, &sys).await.unwrap_err();
        assert_eq!(e.code, "MOCK_BAD_BASEPATH");
        assert_eq!(e.status, 400);

        let bad_port = MockServiceConfig {
            id: "m3".to_string(),
            custom_port: Some(api_port()),
            enabled: true,
            ..Default::default()
        };
        let e = mgr.add_service(bad_port, &cm, &sys).await.unwrap_err();
        assert_eq!(e.code, "MOCK_BAD_PORT");
        assert_eq!(e.status, 400);
    }

    #[tokio::test]
    async fn start_missing_service_errors() {
        let mgr = MockManager::new();
        let cm = temp_cfg();
        let sys = SystemSettings::default();
        let e = mgr.start_service("nope", &cm, &sys).await.unwrap_err();
        assert_eq!(e.code, "MOCK_NOT_FOUND");
        assert_eq!(e.status, 404);
    }
}
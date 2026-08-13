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

use crate::backend::managers::config_manager::ConfigManager;
use crate::backend::types::{MockServiceConfig, SystemSettings};

use super::server::{dispatch, MockServer};

#[derive(Default)]
struct RunningHandle {
    /// 自定义端口的关闭 signal
    shutdown: Option<oneshot::Sender<()>>,
    /// 实际绑定端口
    port: u16,
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
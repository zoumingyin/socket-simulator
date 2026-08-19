//! Repository —— 配置持久化抽象层（v3 P0-1）
//!
//! 把「配置如何存储」从 `ConfigManager` 中抽离为可替换的后端：
//! - `JsonRepository`：单文件 `config.json`（默认，行为兼容旧版）
//! - `SqliteRepository`：SQLite（sqlx），v3 目标存储；P2 起默认启用
//!
//! `ConfigManager` 持 `Arc<dyn Repository>`，**对外同步 API 不变**，
//! 所有 REST/WS handler 零改动。存储切换只在构造时决定。

pub mod json_repo;
pub mod sqlite_repo;

pub use json_repo::JsonRepository;
pub use sqlite_repo::SqliteRepository;

use std::path::Path;

use async_trait::async_trait;

use crate::backend::error::BackendError;
use crate::backend::types::PersistedConfig;

/// 配置存储后端抽象。所有方法均为 async（SQLite 需要；JSON 实现以即时完成的 future 包同步 IO）。
#[async_trait]
pub trait Repository: Send + Sync {
    /// 初始化存储（建表 / 确保目录）。幂等。
    async fn init(&self) -> Result<(), BackendError>;

    /// 载入完整配置。存储为空或损坏时返回 `PersistedConfig::default()`。
    async fn load_config(&self) -> Result<PersistedConfig, BackendError>;

    /// 保存完整配置（全量覆盖）。
    async fn save_config(&self, cfg: &PersistedConfig) -> Result<(), BackendError>;

    /// 从旧 `config.json` 一次性迁移到本存储（仅当本存储为空时调用，幂等）。
    async fn migrate_from_json(&self, json_path: &Path) -> Result<(), BackendError>;
}

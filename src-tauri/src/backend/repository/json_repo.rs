//! JsonRepository —— 单文件 `config.json` 存储（默认后端）
//!
//! 封装旧版 `ConfigManager` 的 JSON 读写逻辑，行为保持一致：
//! 文件缺失/损坏时返回默认配置，不做业务 sanitize（sanitize 留在 `ConfigManager` 内存层）。

use std::path::{Path, PathBuf};

use async_trait::async_trait;

use crate::backend::error::BackendError;
use crate::backend::repository::Repository;
use crate::backend::types::PersistedConfig;

pub struct JsonRepository {
    file: PathBuf,
}

impl JsonRepository {
    pub fn new(config_file: PathBuf) -> Self {
        Self { file: config_file }
    }

    fn ensure_parent(&self) {
        if let Some(p) = self.file.parent() {
            let _ = std::fs::create_dir_all(p);
        }
    }
}

#[async_trait]
impl Repository for JsonRepository {
    async fn init(&self) -> Result<(), BackendError> {
        self.ensure_parent();
        Ok(())
    }

    async fn load_config(&self) -> Result<PersistedConfig, BackendError> {
        match std::fs::read_to_string(&self.file) {
            Ok(s) => Ok(serde_json::from_str::<PersistedConfig>(&s).unwrap_or_default()),
            Err(_) => Ok(PersistedConfig::default()),
        }
    }

    async fn save_config(&self, cfg: &PersistedConfig) -> Result<(), BackendError> {
        self.ensure_parent();
        let json = serde_json::to_string_pretty(cfg)?;
        std::fs::write(&self.file, json)?;
        Ok(())
    }

    async fn migrate_from_json(&self, _json_path: &Path) -> Result<(), BackendError> {
        // 本实现自身就是 JSON 存储，无需迁移（旧 config.json 即当前文件）。
        Ok(())
    }
}

//! SqliteRepository —— SQLite 存储后端（v3 目标存储）
//!
//! 表设计（P0 阶段：配置类实体以「id + JSON 列」存储，保留 serde 直接 round-trip，
//! 零数据转换风险；后续 P1/P2 可逐步规范化）：
//! - `servers(id PK, data TEXT)`        —— Vec<ServerConfig>
//! - `events(id PK, data TEXT)`         —— Vec<EventConfig>
//! - `mock_services(id PK, data TEXT)`  —— Vec<MockServiceConfig>
//! - `singletons(key PK, data TEXT)`    —— system_settings / window_config 单行
//! - `meta(key PK, value TEXT)`         —— version / exported_at
//!
//! `save_config` 为全量覆盖：事务内 DELETE + 逐条 INSERT OR REPLACE。
//! `migrate_from_json` 在 SQLite 为空 **或** 旧 `config.json` 的 `exported_at` 晚于 SQLite
//! 当前 `exported_at` 时，从旧 `config.json` 全量导入（幂等；补全 P2-1 灰度期镜像写失败导致的 SQLite 落后）。

use std::path::Path;

use async_trait::async_trait;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{SqlitePool, Row};

use crate::backend::error::BackendError;
use crate::backend::repository::Repository;
use crate::backend::types::PersistedConfig;

pub struct SqliteRepository {
    pool: SqlitePool,
}

fn to_err(e: sqlx::Error) -> BackendError {
    BackendError::Internal(format!("sqlite: {}", e))
}

impl SqliteRepository {
    /// 打开（或创建）SQLite 数据库。`db_path` 为 `.db` 文件路径。
    pub async fn open(db_path: &Path) -> Result<Self, BackendError> {
        if let Some(p) = db_path.parent() {
            let _ = std::fs::create_dir_all(p);
        }
        let opts = SqliteConnectOptions::new()
            .filename(db_path.to_path_buf())
            .create_if_missing(true);
        // 单连接：配合 ConfigManager 单写者，避免并发写竞争
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .map_err(to_err)?;
        Ok(Self { pool })
    }
}

#[async_trait]
impl Repository for SqliteRepository {
    async fn init(&self) -> Result<(), BackendError> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS servers (id TEXT PRIMARY KEY, data TEXT NOT NULL)",
        )
        .execute(&self.pool)
        .await
        .map_err(to_err)?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS events (id TEXT PRIMARY KEY, data TEXT NOT NULL)",
        )
        .execute(&self.pool)
        .await
        .map_err(to_err)?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS mock_services (id TEXT PRIMARY KEY, data TEXT NOT NULL)",
        )
        .execute(&self.pool)
        .await
        .map_err(to_err)?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS singletons (key TEXT PRIMARY KEY, data TEXT NOT NULL)",
        )
        .execute(&self.pool)
        .await
        .map_err(to_err)?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
        )
        .execute(&self.pool)
        .await
        .map_err(to_err)?;
        Ok(())
    }

    async fn load_config(&self) -> Result<PersistedConfig, BackendError> {
        let mut cfg = PersistedConfig::default();

        let rows = sqlx::query("SELECT data FROM servers")
            .fetch_all(&self.pool)
            .await
            .map_err(to_err)?;
        for r in rows {
            let data: String = r.get("data");
            if let Ok(v) = serde_json::from_str(&data) {
                cfg.servers.push(v);
            }
        }

        let rows = sqlx::query("SELECT data FROM events")
            .fetch_all(&self.pool)
            .await
            .map_err(to_err)?;
        for r in rows {
            let data: String = r.get("data");
            if let Ok(v) = serde_json::from_str(&data) {
                cfg.events.push(v);
            }
        }

        let rows = sqlx::query("SELECT data FROM mock_services")
            .fetch_all(&self.pool)
            .await
            .map_err(to_err)?;
        for r in rows {
            let data: String = r.get("data");
            if let Ok(v) = serde_json::from_str(&data) {
                cfg.mock_services.push(v);
            }
        }

        if let Some(data) = get_singleton(&self.pool, "system_settings").await? {
            if let Ok(v) = serde_json::from_str(&data) {
                cfg.system_settings = v;
            }
        }
        if let Some(data) = get_singleton(&self.pool, "window_config").await? {
            if let Ok(v) = serde_json::from_str(&data) {
                cfg.window_config = v;
            }
        }
        if let Some(v) = get_meta(&self.pool, "version").await? {
            cfg.version = v;
        }
        if let Some(v) = get_meta(&self.pool, "exported_at").await? {
            cfg.exported_at = v;
        }

        Ok(cfg)
    }

    async fn save_config(&self, cfg: &PersistedConfig) -> Result<(), BackendError> {
        let mut tx = self.pool.begin().await.map_err(to_err)?;

        // 全量覆盖：先清空集合表，再逐条写入（ConfigManager 传入完整 Vec）
        sqlx::query("DELETE FROM servers")
            .execute(&mut *tx)
            .await
            .map_err(to_err)?;
        for s in &cfg.servers {
            let data = serde_json::to_string(s)?;
            sqlx::query("INSERT OR REPLACE INTO servers (id, data) VALUES (?, ?)")
                .bind(&s.id)
                .bind(&data)
                .execute(&mut *tx)
                .await
                .map_err(to_err)?;
        }

        sqlx::query("DELETE FROM events")
            .execute(&mut *tx)
            .await
            .map_err(to_err)?;
        for e in &cfg.events {
            let data = serde_json::to_string(e)?;
            sqlx::query("INSERT OR REPLACE INTO events (id, data) VALUES (?, ?)")
                .bind(&e.id)
                .bind(&data)
                .execute(&mut *tx)
                .await
                .map_err(to_err)?;
        }

        sqlx::query("DELETE FROM mock_services")
            .execute(&mut *tx)
            .await
            .map_err(to_err)?;
        for m in &cfg.mock_services {
            let data = serde_json::to_string(m)?;
            sqlx::query("INSERT OR REPLACE INTO mock_services (id, data) VALUES (?, ?)")
                .bind(&m.id)
                .bind(&data)
                .execute(&mut *tx)
                .await
                .map_err(to_err)?;
        }

        sqlx::query("DELETE FROM singletons")
            .execute(&mut *tx)
            .await
            .map_err(to_err)?;
        let sys = serde_json::to_string(&cfg.system_settings)?;
        upsert_singleton(&mut tx, "system_settings", &sys).await?;
        let win = serde_json::to_string(&cfg.window_config)?;
        upsert_singleton(&mut tx, "window_config", &win).await?;

        sqlx::query("DELETE FROM meta")
            .execute(&mut *tx)
            .await
            .map_err(to_err)?;
        upsert_meta(&mut tx, "version", &cfg.version).await?;
        upsert_meta(&mut tx, "exported_at", &cfg.exported_at).await?;

        tx.commit().await.map_err(to_err)?;
        Ok(())
    }

    async fn migrate_from_json(&self, json_path: &Path) -> Result<(), BackendError> {
        // SQLite 当前 servers 数量（空库 = 未迁移过）
        let count: i64 = sqlx::query("SELECT COUNT(*) AS c FROM servers")
            .fetch_one(&self.pool)
            .await
            .map_err(to_err)?
            .get("c");

        // 读取旧 config.json（无文件/损坏 → 跳过，保持现状语义）
        let json_cfg = match std::fs::read_to_string(json_path) {
            Ok(s) => match serde_json::from_str::<PersistedConfig>(&s) {
                Ok(cfg) => cfg,
                Err(_) => return Ok(()), // 损坏的 JSON：跳过迁移
            },
            Err(_) => return Ok(()), // 无旧文件：无需迁移
        };

        // SQLite 当前 exported_at（无则空串）
        let db_exported = get_meta(&self.pool, "exported_at")
            .await?
            .unwrap_or_default();

        // 空库 或 JSON 比 SQLite 更新 → 全量覆盖导入（补全灰度期镜像落后，无数据丢失）
        let should_import = count == 0 || json_cfg.exported_at > db_exported;
        if !should_import {
            return Ok(()); // SQLite 数据更新/相同，不覆盖（无数据丢失）
        }
        self.save_config(&json_cfg).await
    }
}

async fn get_singleton(pool: &SqlitePool, key: &str) -> Result<Option<String>, BackendError> {
    let row = sqlx::query("SELECT data FROM singletons WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await
        .map_err(to_err)?;
    Ok(row.map(|r| r.get::<String, _>("data")))
}

async fn get_meta(pool: &SqlitePool, key: &str) -> Result<Option<String>, BackendError> {
    let row = sqlx::query("SELECT value FROM meta WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await
        .map_err(to_err)?;
    Ok(row.map(|r| r.get::<String, _>("value")))
}

async fn upsert_singleton(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    key: &str,
    data: &str,
) -> Result<(), BackendError> {
    sqlx::query("INSERT OR REPLACE INTO singletons (key, data) VALUES (?, ?)")
        .bind(key)
        .bind(data)
        .execute(&mut **tx)
        .await
        .map_err(to_err)?;
    Ok(())
}

async fn upsert_meta(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    key: &str,
    value: &str,
) -> Result<(), BackendError> {
    sqlx::query("INSERT OR REPLACE INTO meta (key, value) VALUES (?, ?)")
        .bind(key)
        .bind(value)
        .execute(&mut **tx)
        .await
        .map_err(to_err)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::types::*;

    use std::path::PathBuf;

    fn tmp_db() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ssm_sqlite_{}", uuid::Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&dir);
        dir.join("data.db")
    }

    fn sample_config() -> PersistedConfig {
        let mut c = PersistedConfig::default();
        c.servers = vec![ServerConfig {
            id: "a".into(),
            name: "A".into(),
            port: 3000,
            protocol: ProtocolType::Websocket,
            ..Default::default()
        }];
        c.events = vec![EventConfig {
            id: "e1".into(),
            name: "E".into(),
            ..Default::default()
        }];
        c.mock_services = vec![MockServiceConfig {
            id: "m1".into(),
            name: "M".into(),
            ..Default::default()
        }];
        c.system_settings.id = "system".into();
        c.window_config.width = 1024;
        c.version = "2.0.0".into();
        c.exported_at = "2026-08-19T00:00:00Z".into();
        c
    }

    #[tokio::test]
    async fn save_load_roundtrip() {
        let db = tmp_db();
        let repo = SqliteRepository::open(&db).await.unwrap();
        repo.init().await.unwrap();
        let cfg = sample_config();
        repo.save_config(&cfg).await.unwrap();
        let loaded = repo.load_config().await.unwrap();
        assert_eq!(loaded.servers.len(), 1);
        assert_eq!(loaded.servers[0].id, "a");
        assert_eq!(loaded.servers[0].name, "A");
        assert_eq!(loaded.events.len(), 1);
        assert_eq!(loaded.events[0].id, "e1");
        assert_eq!(loaded.mock_services.len(), 1);
        assert_eq!(loaded.mock_services[0].id, "m1");
        assert_eq!(loaded.system_settings.id, "system");
        assert_eq!(loaded.window_config.width, 1024);
        assert_eq!(loaded.version, "2.0.0");
        assert_eq!(loaded.exported_at, "2026-08-19T00:00:00Z");
    }

    #[tokio::test]
    async fn empty_load_is_default() {
        let db = tmp_db();
        let repo = SqliteRepository::open(&db).await.unwrap();
        repo.init().await.unwrap();
        let loaded = repo.load_config().await.unwrap();
        assert!(loaded.servers.is_empty());
        assert_eq!(loaded.version, PersistedConfig::default().version);
    }

    #[tokio::test]
    async fn migrate_from_json_imports_and_is_idempotent() {
        let dir = std::env::temp_dir().join(format!("ssm_mig_{}", uuid::Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&dir);
        let json_path = dir.join("config.json");
        // 用真实 PersistedConfig 序列化生成合法旧 config.json（字段完整，可正确反序列化）
        let json = serde_json::to_string(&sample_config()).unwrap();
        std::fs::write(&json_path, json).unwrap();
        let db = dir.join("data.db");
        let repo = SqliteRepository::open(&db).await.unwrap();
        repo.init().await.unwrap();
        repo.migrate_from_json(&json_path).await.unwrap();
        let loaded = repo.load_config().await.unwrap();
        assert_eq!(loaded.servers.len(), 1);
        assert_eq!(loaded.servers[0].id, "a");
        assert_eq!(loaded.events.len(), 1);
        assert_eq!(loaded.mock_services.len(), 1);
        assert_eq!(loaded.version, "2.0.0");

        // 幂等：连续两次迁移，第二次应因 servers 表非空而跳过（不重复导入）
        repo.migrate_from_json(&json_path).await.unwrap();
        let reloaded = repo.load_config().await.unwrap();
        assert_eq!(
            reloaded.servers.len(),
            1,
            "迁移应幂等：重复迁移不应重复导入（仍为 1 条）"
        );
    }

    /// 更新的 JSON 应覆盖「过时的 SQLite」（exported_at 比较）
    #[tokio::test]
    async fn migrate_overrides_stale_db_when_json_is_newer() {
        let dir = std::env::temp_dir().join(format!("ssm_mig_newer_{}", uuid::Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&dir);
        let db = dir.join("data.db");

        // 先写入一个「旧」SQLite（exported_at 较早）
        let mut old = sample_config();
        old.exported_at = "2020-01-01T00:00:00Z".into();
        old.servers = vec![ServerConfig {
            id: "stale".into(),
            name: "Stale".into(),
            port: 3000,
            protocol: ProtocolType::Websocket,
            ..Default::default()
        }];
        {
            let repo = SqliteRepository::open(&db).await.unwrap();
            repo.init().await.unwrap();
            repo.save_config(&old).await.unwrap();
        }

        // 造一个 exported_at 更新的 config.json
        let mut newer = sample_config();
        newer.exported_at = "2099-01-01T00:00:00Z".into();
        newer.servers = vec![ServerConfig {
            id: "fresh".into(),
            name: "Fresh".into(),
            port: 3000,
            protocol: ProtocolType::Websocket,
            ..Default::default()
        }];
        let json_path = dir.join("config.json");
        std::fs::write(&json_path, serde_json::to_string(&newer).unwrap()).unwrap();

        let repo = SqliteRepository::open(&db).await.unwrap();
        repo.migrate_from_json(&json_path).await.unwrap();
        let loaded = repo.load_config().await.unwrap();
        assert_eq!(loaded.servers.len(), 1, "应被更新的 JSON 覆盖");
        assert_eq!(loaded.servers[0].id, "fresh");
    }

    /// 较旧的 JSON 不应覆盖「更新的 SQLite」（无数据丢失反向验证）
    #[tokio::test]
    async fn migrate_keeps_newer_db_when_json_is_stale() {
        let dir = std::env::temp_dir().join(format!("ssm_mig_older_{}", uuid::Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&dir);
        let db = dir.join("data.db");

        // 先写入一个「新」SQLite（exported_at 较晚）
        let mut fresh = sample_config();
        fresh.exported_at = "2099-01-01T00:00:00Z".into();
        fresh.servers = vec![ServerConfig {
            id: "keep".into(),
            name: "Keep".into(),
            port: 3000,
            protocol: ProtocolType::Websocket,
            ..Default::default()
        }];
        {
            let repo = SqliteRepository::open(&db).await.unwrap();
            repo.init().await.unwrap();
            repo.save_config(&fresh).await.unwrap();
        }

        // 造一个 exported_at 更旧的 config.json
        let mut stale = sample_config();
        stale.exported_at = "2020-01-01T00:00:00Z".into();
        stale.servers = vec![ServerConfig {
            id: "stale".into(),
            name: "Stale".into(),
            port: 3000,
            protocol: ProtocolType::Websocket,
            ..Default::default()
        }];
        let json_path = dir.join("config.json");
        std::fs::write(&json_path, serde_json::to_string(&stale).unwrap()).unwrap();

        let repo = SqliteRepository::open(&db).await.unwrap();
        repo.migrate_from_json(&json_path).await.unwrap();
        let loaded = repo.load_config().await.unwrap();
        assert_eq!(loaded.servers.len(), 1, "不应被旧 JSON 覆盖（无数据丢失）");
        assert_eq!(loaded.servers[0].id, "keep");
    }
}

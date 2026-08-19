//! v3 P0-3 审计日志
//!
//! 关键管理操作（启停服务 / 改配置 / 删规则 / 导入配置 / 清日志）写入 SQLite 审计表，
//! 提供分页 + 过滤查询（`GET /api/audit/logs`），满足「关键操作可查」。
//!
//! - `AuditManager`：独立 `audit.db`（与 `auth.db` 互不干扰），写后即查
//! - `AuditEntry`：单条审计记录（actor / action / target / detail / result）
//! - `record_audit` 辅助：handlers 埋点统一入口，自动从请求头解析 actor 角色
//!
//! 鉴权关闭时（默认）actor 记为 `local`；开启时优先用 Bearer token 解析角色。

use std::path::Path;
use std::sync::Arc;

use axum::http::header::AUTHORIZATION;
use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

use crate::backend::auth::Role;
use crate::backend::error::BackendError;
use crate::backend::types::now_rfc3339;

/// 单条审计记录
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuditEntry {
    pub id: String,
    /// RFC3339 时间戳
    pub timestamp: String,
    /// 操作者：admin / viewer / local（鉴权关闭或自举时）
    pub actor: String,
    /// 操作名：server_start / server_update / mock_remove ...
    pub action: String,
    /// 对象类型：server / mock / event / config / log / client
    pub target_type: String,
    pub target_id: Option<String>,
    /// 附加详情（JSON）
    pub detail: Option<Value>,
    /// 结果：success / failure
    pub result: String,
}

/// 查询参数（分页 + 可选过滤）
#[derive(Debug, Clone, Deserialize, Default, utoipa::IntoParams, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuditQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
    /// 按操作名精确过滤
    #[serde(default)]
    pub action: Option<String>,
    /// 按对象类型精确过滤
    #[serde(default)]
    pub target_type: Option<String>,
}

fn default_limit() -> i64 {
    100
}

/// 分页结果
#[derive(Debug, Clone, Serialize, specta::Type, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuditPage {
    pub total: i64,
    pub items: Vec<AuditEntry>,
}

/// 审计管理器：SQLite 写入 + 查询
pub struct AuditManager {
    pool: SqlitePool,
}

impl AuditManager {
    async fn new_async(data_dir: &Path) -> Self {
        let db_path = data_dir.join("audit.db");
        let opts = SqliteConnectOptions::new()
            .filename(&db_path)
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .expect("AuditManager: 打开 audit.db 失败");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS audit_logs (
                id TEXT PRIMARY KEY,
                timestamp TEXT NOT NULL,
                actor TEXT NOT NULL,
                action TEXT NOT NULL,
                target_type TEXT NOT NULL,
                target_id TEXT,
                detail TEXT,
                result TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("AuditManager: 建 audit_logs 表失败");

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_audit_action ON audit_logs (action)",
        )
        .execute(&pool)
        .await
        .expect("AuditManager: 建 action 索引失败");

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON audit_logs (timestamp DESC)",
        )
        .execute(&pool)
        .await
        .expect("AuditManager: 建 timestamp 索引失败");

        Self { pool }
    }

    /// 同步构造：独立 OS 线程 + 独立 multi_thread runtime（与 AuthManager 同模式）
    pub fn new_blocking(data_dir: &Path) -> Arc<Self> {
        let dir = data_dir.to_path_buf();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("AuditManager: 创建 runtime 失败");
            let mgr = rt.block_on(Self::new_async(dir.as_path()));
            let _ = tx.send(mgr);
        });
        Arc::new(rx.recv().expect("AuditManager: 初始化异常"))
    }

    /// 写入一条审计记录
    pub async fn record(&self, entry: &AuditEntry) -> Result<(), BackendError> {
        let detail = entry
            .detail
            .as_ref()
            .map(|v| v.to_string())
            .unwrap_or_default();
        sqlx::query(
            "INSERT INTO audit_logs (id, timestamp, actor, action, target_type, target_id, detail, result)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&entry.id)
        .bind(&entry.timestamp)
        .bind(&entry.actor)
        .bind(&entry.action)
        .bind(&entry.target_type)
        .bind(&entry.target_id)
        .bind(&detail)
        .bind(&entry.result)
        .execute(&self.pool)
        .await
        .map_err(|e| BackendError::Internal(e.to_string()))?;
        Ok(())
    }

    /// 分页查询（按时间倒序），支持 action / targetType 过滤
    pub async fn query(&self, q: &AuditQuery) -> Result<AuditPage, BackendError> {
        let mut where_sql = String::from(" WHERE 1=1");
        if q.action.is_some() {
            where_sql.push_str(" AND action = ?");
        }
        if q.target_type.is_some() {
            where_sql.push_str(" AND target_type = ?");
        }
        let total_sql = format!("SELECT COUNT(*) FROM audit_logs{}", where_sql);
        let mut total_q = sqlx::query_scalar::<_, i64>(&total_sql);
        if let Some(a) = &q.action {
            total_q = total_q.bind(a);
        }
        if let Some(t) = &q.target_type {
            total_q = total_q.bind(t);
        }
        let total = total_q
            .fetch_one(&self.pool)
            .await
            .map_err(|e| BackendError::Internal(e.to_string()))?;

        let list_sql = format!(
            "SELECT id, timestamp, actor, action, target_type, target_id, detail, result
             FROM audit_logs{} ORDER BY timestamp DESC LIMIT ? OFFSET ?",
            where_sql
        );
        let mut list_q = sqlx::query(&list_sql);
        if let Some(a) = &q.action {
            list_q = list_q.bind(a);
        }
        if let Some(t) = &q.target_type {
            list_q = list_q.bind(t);
        }
        let rows = list_q
            .bind(q.limit)
            .bind(q.offset)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| BackendError::Internal(e.to_string()))?;

        let items = rows
            .into_iter()
            .map(|row| AuditRow {
                id: row.get(0),
                timestamp: row.get(1),
                actor: row.get(2),
                action: row.get(3),
                target_type: row.get(4),
                target_id: row.get(5),
                detail: row.get(6),
                result: row.get(7),
            })
            .map(AuditRow::into_entry)
            .collect();
        Ok(AuditPage { total, items })
    }
}

/// SQLite 行 → AuditEntry（detail 文本还原为 JSON）
struct AuditRow {
    id: String,
    timestamp: String,
    actor: String,
    action: String,
    target_type: String,
    target_id: Option<String>,
    detail: String,
    result: String,
}

impl AuditRow {
    fn into_entry(self) -> AuditEntry {
        let detail = if self.detail.is_empty() {
            None
        } else {
            serde_json::from_str(&self.detail).ok()
        };
        AuditEntry {
            id: self.id,
            timestamp: self.timestamp,
            actor: self.actor,
            action: self.action,
            target_type: self.target_type,
            target_id: self.target_id,
            detail,
            result: self.result,
        }
    }
}

// ==================== handlers 埋点辅助 ====================

/// 从请求头解析操作者角色：
/// - 鉴权关闭 → Admin（本地管理）
/// - 鉴权开启 → 按 Bearer token 校验；无效则 None
pub async fn actor_role(b: &crate::backend::app::Backend, headers: &HeaderMap) -> Option<Role> {
    if !b.auth.enabled() {
        return Some(Role::Admin);
    }
    let token = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(parse_bearer)?;
    b.auth.verify(&token).await
}

fn parse_bearer(header: &str) -> Option<String> {
    let mut parts = header.split_whitespace();
    match (parts.next(), parts.next()) {
        (Some(scheme), Some(token)) if scheme.eq_ignore_ascii_case("Bearer") => {
            Some(token.to_string())
        }
        _ => None,
    }
}

/// 关键操作埋点统一入口（handlers 调用）。失败不阻断业务（审计为旁路）。
pub async fn record_audit(
    b: &crate::backend::app::Backend,
    role: Option<Role>,
    action: &str,
    target_type: &str,
    target_id: Option<String>,
    detail: Value,
    success: bool,
) {
    let entry = AuditEntry {
        id: uuid::Uuid::new_v4().to_string(),
        timestamp: now_rfc3339(),
        actor: role
            .map(|r| r.as_str().to_string())
            .unwrap_or_else(|| "local".to_string()),
        action: action.to_string(),
        target_type: target_type.to_string(),
        target_id,
        detail: if detail.is_null() { None } else { Some(detail) },
        result: if success { "success" } else { "failure" }.into(),
    };
    let _ = b.audit.record(&entry).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir() -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("ssm_audit_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn sample(action: &str, target_type: &str) -> AuditEntry {
        AuditEntry {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: now_rfc3339(),
            actor: "admin".to_string(),
            action: action.to_string(),
            target_type: target_type.to_string(),
            target_id: Some("srv-1".to_string()),
            detail: Some(serde_json::json!({ "note": "test" })),
            result: "success".to_string(),
        }
    }

    #[tokio::test]
    async fn record_and_query_roundtrip() {
        let dir = tmp_dir();
        let mgr = AuditManager::new_async(dir.as_path()).await;
        let e = sample("server_start", "server");
        mgr.record(&e).await.unwrap();

        let page = mgr
            .query(&AuditQuery {
                limit: 10,
                offset: 0,
                action: None,
                target_type: None,
            })
            .await
            .unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].action, "server_start");
        assert_eq!(page.items[0].actor, "admin");
        assert_eq!(page.items[0].target_type, "server");
        assert_eq!(page.items[0].target_id.as_deref(), Some("srv-1"));
        assert_eq!(page.items[0].detail, Some(serde_json::json!({ "note": "test" })));
    }

    #[tokio::test]
    async fn query_filters_by_action_and_type() {
        let dir = tmp_dir();
        let mgr = AuditManager::new_async(dir.as_path()).await;
        mgr.record(&sample("server_start", "server")).await.unwrap();
        mgr.record(&sample("server_stop", "server")).await.unwrap();
        mgr.record(&sample("mock_remove", "mock")).await.unwrap();

        let page = mgr
            .query(&AuditQuery {
                limit: 10,
                offset: 0,
                action: Some("server_stop".into()),
                target_type: None,
            })
            .await
            .unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].action, "server_stop");

        let page = mgr
            .query(&AuditQuery {
                limit: 10,
                offset: 0,
                action: None,
                target_type: Some("mock".into()),
            })
            .await
            .unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].target_type, "mock");
    }

    #[tokio::test]
    async fn pagination_returns_offset_items() {
        let dir = tmp_dir();
        let mgr = AuditManager::new_async(dir.as_path()).await;
        for i in 0..5 {
            let mut e = sample("server_start", "server");
            e.timestamp = format!("2026-08-19T10:00:0{}Z", i);
            e.id = format!("id-{}", i);
            mgr.record(&e).await.unwrap();
        }
        let page = mgr
            .query(&AuditQuery {
                limit: 2,
                offset: 2,
                action: None,
                target_type: None,
            })
            .await
            .unwrap();
        assert_eq!(page.total, 5);
        assert_eq!(page.items.len(), 2);
    }

    #[test]
    fn bearer_parsing() {
        assert_eq!(parse_bearer("Bearer abc"), Some("abc".to_string()));
        assert_eq!(parse_bearer("Basic xyz"), None);
        assert_eq!(parse_bearer("abc"), None);
        assert_eq!(parse_bearer(""), None);
    }
}

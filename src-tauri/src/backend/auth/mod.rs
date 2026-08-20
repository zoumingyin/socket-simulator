//! v3 P0-2 鉴权基础设施（可插拔，默认关闭以保证零回归）
//!
//! - `Role`：权限角色（Admin 全权 / Viewer 只读）
//! - `AuthManager`：admin token 生成 + SQLite 持久化 + 校验
//! - `auth_middleware`：按开关拦截 Bearer token；豁免 bootstrap / health / 管理 WS / 非 API 静态资源
//! - `bootstrap` 端点：本地回环自举返回 admin token（供前端 P1 接入鉴权）

use std::path::Path;
use std::sync::Arc;

use axum::extract::Request;
use axum::extract::State;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

/// 权限角色
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// 全权（默认 admin token 拥有）
    #[default]
    Admin,
    /// 只读（预留，未来可限制写操作）
    Viewer,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Admin => "admin",
            Role::Viewer => "viewer",
        }
    }
    pub fn from_str(s: &str) -> Role {
        match s {
            "viewer" => Role::Viewer,
            _ => Role::Admin,
        }
    }
}

/// 本地自举响应体
#[derive(Serialize)]
pub struct BootstrapResponse {
    pub token: String,
    pub role: Role,
}

/// 鉴权管理器：admin token 生成 / 持久化 / 校验
pub struct AuthManager {
    pool: SqlitePool,
    admin_token: String,
    enabled: bool,
}

impl AuthManager {
    async fn new_async(data_dir: &Path) -> Self {
        let db_path = data_dir.join("auth.db");
        let opts = SqliteConnectOptions::new()
            .filename(&db_path)
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .expect("AuthManager: 打开 auth.db 失败");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS tokens (token TEXT PRIMARY KEY, role TEXT NOT NULL)",
        )
        .execute(&pool)
        .await
        .expect("AuthManager: 建 tokens 表失败");

        // 仅当无 admin token 时生成（幂等：重新打开复用既有 token）
        let admin_token = match sqlx::query_scalar::<_, String>(
            "SELECT token FROM tokens WHERE role = 'admin' LIMIT 1",
        )
        .fetch_optional(&pool)
        .await
        .expect("AuthManager: 查询 admin token 失败")
        {
            Some(t) => t,
            None => {
                let t = generate_token();
                sqlx::query("INSERT INTO tokens (token, role) VALUES (?, 'admin')")
                    .bind(&t)
                    .execute(&pool)
                    .await
                    .expect("AuthManager: 写入 admin token 失败");
                t
            }
        };

        // 默认关闭鉴权（零回归）；通过 `SSM_AUTH_ENABLED=1` 启用
        let enabled = std::env::var("SSM_AUTH_ENABLED")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        Self {
            pool,
            admin_token,
            enabled,
        }
    }

    /// 同步构造：独立 OS 线程 + 独立 multi_thread runtime 驱动 async init，
    /// 规避「runtime within runtime」与单 worker 下 `block_in_place` 报错。
    pub fn new_blocking(data_dir: &Path) -> Arc<Self> {
        let dir = data_dir.to_path_buf();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("AuthManager: 创建 runtime 失败");
            let mgr = rt.block_on(Self::new_async(dir.as_path()));
            let _ = tx.send(mgr);
        });
        Arc::new(rx.recv().expect("AuthManager: 初始化异常"))
    }

    /// 当前 admin token（本地回环自举用）
    pub fn admin_token(&self) -> &str {
        &self.admin_token
    }

    /// 鉴权开关
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// 校验 token，命中返回角色
    pub async fn verify(&self, token: &str) -> Option<Role> {
        if token.is_empty() {
            return None;
        }
        let role: Option<String> = sqlx::query_scalar("SELECT role FROM tokens WHERE token = ?")
            .bind(token)
            .fetch_optional(&self.pool)
            .await
            .ok()
            .flatten();
        role.map(|r| Role::from_str(&r))
    }
}

fn generate_token() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// 是否豁免鉴权（本地自举端点 / 健康检查 / 管理 WS / 非 API 静态资源）
pub fn is_exempt(path: &str) -> bool {
    path == "/api/auth/bootstrap"
        || path == "/api/health"
        || path == crate::backend::constants::ADMIN_WS_PATH
        || !path.starts_with("/api/")
}

/// 从 `Authorization` 头值解析 Bearer token（头值形如 `Bearer <token>`）
fn bearer_from_header(header: &str) -> Option<String> {
    let mut parts = header.split_whitespace();
    match (parts.next(), parts.next()) {
        (Some(scheme), Some(token)) if scheme.eq_ignore_ascii_case("Bearer") => {
            Some(token.to_string())
        }
        _ => None,
    }
}

/// 从 `Authorization` 头提取 Bearer token
pub fn extract_bearer(req: &Request) -> Option<String> {
    let header = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    bearer_from_header(header)
}

/// 只读方法判定：viewer 角色仅允许读取类方法
fn is_readonly_method(method: &axum::http::Method) -> bool {
    matches!(
        method,
        &axum::http::Method::GET | &axum::http::Method::HEAD | &axum::http::Method::OPTIONS
    )
}

/// viewer 只读强制：viewer + 非只读方法 → 返回 403 响应（None 表示放行）
pub fn viewer_write_forbidden(role: Role, method: &axum::http::Method) -> Option<Response> {
    if role == Role::Viewer && !is_readonly_method(method) {
        Some((
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": "forbidden",
                "message": "viewer 角色只读，禁止写操作"
            })),
        )
            .into_response())
    } else {
        None
    }
}

/// 鉴权中间件：enabled 时校验 Bearer token（viewer 只读强制），否则直通
pub async fn auth_middleware(State(auth): State<Arc<AuthManager>>, req: Request, next: Next) -> Response {
    if !auth.enabled() {
        return next.run(req).await;
    }
    let path = req.uri().path().to_string();
    if is_exempt(&path) {
        return next.run(req).await;
    }
    if let Some(token) = extract_bearer(&req) {
        if let Some(role) = auth.verify(&token).await {
            // viewer 只读：写操作直接 403（方法判定先于 req move）
            if let Some(resp) = viewer_write_forbidden(role, req.method()) {
                return resp;
            }
            return next.run(req).await;
        }
    }
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({
            "error": "unauthorized",
            "message": "缺少或无效的 Authorization Bearer token"
        })),
    )
        .into_response()
}

/// 本地自举端点：返回 admin token（供前端 P1 接入鉴权）
#[utoipa::path(
    get,
    path = "/api/auth/bootstrap",
    responses((status = 200, description = "返回 admin token 与角色（回环自举，免鉴权）"))
)]
pub async fn bootstrap(State(auth): State<Arc<AuthManager>>) -> impl IntoResponse {
    Json(BootstrapResponse {
        token: auth.admin_token().to_string(),
        role: Role::Admin,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir() -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("ssm_auth_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[tokio::test]
    async fn init_generates_and_persists_admin_token() {
        let dir = tmp_dir();
        let mgr = AuthManager::new_async(dir.as_path()).await;
        let t = mgr.admin_token().to_string();
        assert!(!t.is_empty());
        // 重新打开应读到同一 token（持久化生效）
        drop(mgr);
        let mgr2 = AuthManager::new_async(dir.as_path()).await;
        assert_eq!(mgr2.admin_token(), t);
    }

    #[tokio::test]
    async fn verify_accepts_admin_rejects_garbage() {
        let dir = tmp_dir();
        let mgr = AuthManager::new_async(dir.as_path()).await;
        let t = mgr.admin_token().to_string();
        assert_eq!(mgr.verify(&t).await, Some(Role::Admin));
        assert_eq!(mgr.verify("garbage").await, None);
        assert_eq!(mgr.verify("").await, None);
    }

    #[test]
    fn exempt_paths_and_bearer_parsing() {
        assert!(is_exempt("/api/auth/bootstrap"));
        assert!(is_exempt("/api/health"));
        assert!(is_exempt(crate::backend::constants::ADMIN_WS_PATH));
        assert!(is_exempt("/"));
        assert!(is_exempt("/assets/app.js"));
        assert!(!is_exempt("/api/servers"));
        assert_eq!(bearer_from_header("Bearer abc"), Some("abc".to_string()));
        assert_eq!(bearer_from_header("Basic xyz"), None);
        assert_eq!(bearer_from_header("abc"), None);
        assert_eq!(bearer_from_header(""), None);
    }

    #[test]
    fn readonly_method_judgment() {
        use axum::http::Method;
        assert!(is_readonly_method(&Method::GET));
        assert!(is_readonly_method(&Method::HEAD));
        assert!(is_readonly_method(&Method::OPTIONS));
        assert!(!is_readonly_method(&Method::POST));
        assert!(!is_readonly_method(&Method::PUT));
        assert!(!is_readonly_method(&Method::PATCH));
        assert!(!is_readonly_method(&Method::DELETE));
    }

    #[test]
    fn viewer_write_forbidden_blocks_writes_allows_reads() {
        use axum::http::Method;
        // viewer + 写 → 403
        let resp = viewer_write_forbidden(Role::Viewer, &Method::POST)
            .expect("viewer POST 应被拦截");
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        assert!(viewer_write_forbidden(Role::Viewer, &Method::PUT).is_some());
        assert!(viewer_write_forbidden(Role::Viewer, &Method::DELETE).is_some());
        // viewer + 读 → 放行
        assert!(viewer_write_forbidden(Role::Viewer, &Method::GET).is_none());
        assert!(viewer_write_forbidden(Role::Viewer, &Method::HEAD).is_none());
        // admin 全权（含写）
        assert!(viewer_write_forbidden(Role::Admin, &Method::POST).is_none());
        assert!(viewer_write_forbidden(Role::Admin, &Method::DELETE).is_none());
    }

    #[tokio::test]
    async fn verify_returns_viewer_role_for_viewer_token() {
        let dir = tmp_dir();
        let mgr = AuthManager::new_async(dir.as_path()).await;
        sqlx::query("INSERT INTO tokens (token, role) VALUES ('viewer-token', 'viewer')")
            .execute(&mgr.pool)
            .await
            .unwrap();
        assert_eq!(mgr.verify("viewer-token").await, Some(Role::Viewer));
        assert_eq!(mgr.verify(&mgr.admin_token()).await, Some(Role::Admin));
    }
}

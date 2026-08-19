//! 受管服务管理处理函数（/api/server/*、/api/servers）

use axum::extract::{Json, State};
use axum::http::HeaderMap;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::backend::api::handlers::{audit_log, err, ok, ok_msg, ok_msg_only, Resp};
use crate::backend::state::AppState;
use crate::backend::types::*;

use uuid::Uuid;

#[derive(Deserialize, utoipa::ToSchema)]
pub(crate) struct ServerId {
    id: String,
}

/// 服务配置摘要（审计 detail 用，避免记录完整配置）
fn cfg_summary(c: &ServerConfig) -> Value {
    json!({
        "name": c.name,
        "protocol": c.protocol,
        "port": c.port,
        "ip": c.ip,
    })
}

#[utoipa::path(
    get,
    path = "/api/servers",
    responses((status = 200, description = "OK"))
)]
pub async fn get_servers(State(b): State<AppState>) -> Resp {
    let configs = b.config.get_servers();
    let runtimes = b.services.get_runtimes();
    ok(serde_json::json!({ "configs": configs, "runtimes": runtimes }))
}

#[utoipa::path(
    get,
    path = "/api/server/list",
    responses((status = 200, description = "OK"))
)]
pub async fn get_server_list(State(b): State<AppState>) -> Resp {
    ok(serde_json::to_value(b.config.get_servers()).unwrap_or(Value::Null))
}

#[utoipa::path(
    get,
    path = "/api/server/runtimes",
    responses((status = 200, description = "OK"))
)]
pub async fn get_server_runtimes(State(b): State<AppState>) -> Resp {
    ok(serde_json::to_value(b.services.get_runtimes()).unwrap_or(Value::Null))
}

#[utoipa::path(
    post,
    path = "/api/server/add",
    request_body = ServerConfig,
    responses((status = 200, description = "OK"))
)]
pub async fn server_add(
    State(b): State<AppState>,
    headers: HeaderMap,
    Json(mut body): Json<ServerConfig>,
) -> Resp {
    if body.id.is_empty() {
        body.id = Uuid::new_v4().to_string();
    }
    if body.created_at.is_empty() {
        body.created_at = now_rfc3339();
    }
    if body.updated_at.is_empty() {
        body.updated_at = now_rfc3339();
    }
    b.services.register_server(body.clone());
    let mut servers = b.config.get_servers();
    servers.push(body.clone());
    b.config.save_servers(servers);
    audit_log(
        &b,
        &headers,
        "server_add",
        "server",
        Some(body.id.clone()),
        cfg_summary(&body),
        true,
    )
    .await;
    ok_msg(
        serde_json::to_value(body).unwrap_or(Value::Null),
        "添加成功",
    )
}

#[utoipa::path(
    post,
    path = "/api/server/update",
    request_body = ServerConfig,
    responses((status = 200, description = "OK"))
)]
pub async fn server_update(
    State(b): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ServerConfig>,
) -> Resp {
    let old = b.config.get_server_by_id(&body.id);
    b.services.register_server(body.clone());
    let mut servers = b.config.get_servers();
    match servers.iter().position(|s| s.id == body.id) {
        Some(i) => servers[i] = body.clone(),
        None => servers.push(body.clone()),
    }
    b.config.save_servers(servers);
    audit_log(
        &b,
        &headers,
        "server_update",
        "server",
        Some(body.id.clone()),
        json!({ "summary": cfg_summary(&body), "changedFrom": old.map(|o| cfg_summary(&o)) }),
        true,
    )
    .await;
    ok_msg_only("更新成功")
}

#[utoipa::path(
    post,
    path = "/api/server/remove",
    request_body = ServerId,
    responses((status = 200, description = "OK"))
)]
pub async fn server_remove(
    State(b): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ServerId>,
) -> Resp {
    let ok_remove = b.services.remove_server(&body.id);
    if !ok_remove {
        audit_log(
            &b,
            &headers,
            "server_remove",
            "server",
            Some(body.id.clone()),
            json!({ "error": "SERVER_RUNNING" }),
            false,
        )
        .await;
        return err("SERVER_RUNNING", "服务正在运行，无法删除".into(), 400);
    }
    let servers = b
        .config
        .get_servers()
        .into_iter()
        .filter(|s| s.id != body.id)
        .collect();
    b.config.save_servers(servers);
    audit_log(
        &b,
        &headers,
        "server_remove",
        "server",
        Some(body.id.clone()),
        json!({}),
        true,
    )
    .await;
    ok_msg_only("删除成功")
}

#[utoipa::path(
    post,
    path = "/api/server/start",
    request_body = ServerId,
    responses((status = 200, description = "OK"))
)]
pub async fn server_start(
    State(b): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ServerId>,
) -> Resp {
    let result = b.services.start(body.id.clone()).await;
    let (success, detail) = match &result {
        Ok(()) => (true, json!({})),
        Err(e) => (false, json!({ "error": e.to_string() })),
    };
    audit_log(
        &b,
        &headers,
        "server_start",
        "server",
        Some(body.id.clone()),
        detail,
        success,
    )
    .await;
    match result {
        Ok(()) => ok_msg_only("启动成功"),
        Err(e) => err(e.error_code(), e.to_string(), e.status_code()),
    }
}

#[utoipa::path(
    post,
    path = "/api/server/stop",
    request_body = ServerId,
    responses((status = 200, description = "OK"))
)]
pub async fn server_stop(
    State(b): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ServerId>,
) -> Resp {
    let result = b.services.stop(&body.id).await;
    let (success, detail) = match &result {
        Ok(()) => (true, json!({})),
        Err(e) => (false, json!({ "error": e.to_string() })),
    };
    audit_log(
        &b,
        &headers,
        "server_stop",
        "server",
        Some(body.id.clone()),
        detail,
        success,
    )
    .await;
    match result {
        Ok(()) => ok_msg_only("停止成功"),
        Err(e) => err(e.error_code(), e.to_string(), e.status_code()),
    }
}

#[utoipa::path(
    post,
    path = "/api/server/restart",
    request_body = ServerId,
    responses((status = 200, description = "OK"))
)]
pub async fn server_restart(
    State(b): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ServerId>,
) -> Resp {
    let result = b.services.restart(body.id.clone()).await;
    let (success, detail) = match &result {
        Ok(()) => (true, json!({})),
        Err(e) => (false, json!({ "error": e.to_string() })),
    };
    audit_log(
        &b,
        &headers,
        "server_restart",
        "server",
        Some(body.id.clone()),
        detail,
        success,
    )
    .await;
    match result {
        Ok(()) => ok_msg_only("重启成功"),
        Err(e) => err(e.error_code(), e.to_string(), e.status_code()),
    }
}

#[utoipa::path(
    post,
    path = "/api/server/start-all",
    responses((status = 200, description = "OK"))
)]
pub async fn server_start_all(State(b): State<AppState>, headers: HeaderMap) -> Resp {
    let result = b.services.start_all().await;
    let (success, detail) = match &result {
        Ok(()) => (true, json!({})),
        Err(e) => (false, json!({ "error": e.to_string() })),
    };
    audit_log(&b, &headers, "server_start_all", "server", None, detail, success).await;
    match result {
        Ok(()) => ok_msg_only("全部启动成功"),
        Err(e) => err(e.error_code(), e.to_string(), e.status_code()),
    }
}

#[utoipa::path(
    post,
    path = "/api/server/stop-all",
    responses((status = 200, description = "OK"))
)]
pub async fn server_stop_all(State(b): State<AppState>, headers: HeaderMap) -> Resp {
    let result = b.services.stop_all().await;
    let (success, detail) = match &result {
        Ok(()) => (true, json!({})),
        Err(e) => (false, json!({ "error": e.to_string() })),
    };
    audit_log(&b, &headers, "server_stop_all", "server", None, detail, success).await;
    match result {
        Ok(()) => ok_msg_only("全部停止成功"),
        Err(e) => err(e.error_code(), e.to_string(), e.status_code()),
    }
}

#[utoipa::path(
    post,
    path = "/api/server/restart-all",
    responses((status = 200, description = "OK"))
)]
pub async fn server_restart_all(State(b): State<AppState>, headers: HeaderMap) -> Resp {
    let result = b.services.restart_all().await;
    let (success, detail) = match &result {
        Ok(()) => (true, json!({})),
        Err(e) => (false, json!({ "error": e.to_string() })),
    };
    audit_log(&b, &headers, "server_restart_all", "server", None, detail, success).await;
    match result {
        Ok(()) => ok_msg_only("全部重启成功"),
        Err(e) => err(e.error_code(), e.to_string(), e.status_code()),
    }
}

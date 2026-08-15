//! 受管服务管理处理函数（/api/server/*、/api/servers）

use axum::extract::{Json, State};
use serde::Deserialize;
use serde_json::Value;

use crate::backend::api::handlers::{err, ok, ok_msg, ok_msg_only, Resp};
use crate::backend::state::AppState;
use crate::backend::types::*;

use uuid::Uuid;

#[derive(Deserialize)]
pub(crate) struct ServerId {
    id: String,
}

pub async fn get_servers(State(b): State<AppState>) -> Resp {
    let configs = b.config.get_servers();
    let runtimes = b.services.get_runtimes();
    ok(serde_json::json!({ "configs": configs, "runtimes": runtimes }))
}

pub async fn get_server_list(State(b): State<AppState>) -> Resp {
    ok(serde_json::to_value(b.config.get_servers()).unwrap_or(Value::Null))
}

pub async fn get_server_runtimes(State(b): State<AppState>) -> Resp {
    ok(serde_json::to_value(b.services.get_runtimes()).unwrap_or(Value::Null))
}

pub async fn server_add(State(b): State<AppState>, Json(mut body): Json<ServerConfig>) -> Resp {
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
    ok_msg(
        serde_json::to_value(body).unwrap_or(Value::Null),
        "添加成功",
    )
}

pub async fn server_update(State(b): State<AppState>, Json(body): Json<ServerConfig>) -> Resp {
    b.services.register_server(body.clone());
    let mut servers = b.config.get_servers();
    match servers.iter().position(|s| s.id == body.id) {
        Some(i) => servers[i] = body,
        None => servers.push(body),
    }
    b.config.save_servers(servers);
    ok_msg_only("更新成功")
}

pub async fn server_remove(State(b): State<AppState>, Json(body): Json<ServerId>) -> Resp {
    if !b.services.remove_server(&body.id) {
        return err("SERVER_RUNNING", "服务正在运行，无法删除".into(), 400);
    }
    let servers = b
        .config
        .get_servers()
        .into_iter()
        .filter(|s| s.id != body.id)
        .collect();
    b.config.save_servers(servers);
    ok_msg_only("删除成功")
}

pub async fn server_start(State(b): State<AppState>, Json(body): Json<ServerId>) -> Resp {
    match b.services.start(body.id.clone()).await {
        Ok(()) => ok_msg_only("启动成功"),
        Err(e) => err(e.error_code(), e.to_string(), e.status_code()),
    }
}

pub async fn server_stop(State(b): State<AppState>, Json(body): Json<ServerId>) -> Resp {
    match b.services.stop(&body.id).await {
        Ok(()) => ok_msg_only("停止成功"),
        Err(e) => err(e.error_code(), e.to_string(), e.status_code()),
    }
}

pub async fn server_restart(State(b): State<AppState>, Json(body): Json<ServerId>) -> Resp {
    match b.services.restart(body.id.clone()).await {
        Ok(()) => ok_msg_only("重启成功"),
        Err(e) => err(e.error_code(), e.to_string(), e.status_code()),
    }
}

pub async fn server_start_all(State(b): State<AppState>) -> Resp {
    match b.services.start_all().await {
        Ok(()) => ok_msg_only("全部启动成功"),
        Err(e) => err(e.error_code(), e.to_string(), e.status_code()),
    }
}

pub async fn server_stop_all(State(b): State<AppState>) -> Resp {
    match b.services.stop_all().await {
        Ok(()) => ok_msg_only("全部停止成功"),
        Err(e) => err(e.error_code(), e.to_string(), e.status_code()),
    }
}

pub async fn server_restart_all(State(b): State<AppState>) -> Resp {
    match b.services.restart_all().await {
        Ok(()) => ok_msg_only("全部重启成功"),
        Err(e) => err(e.error_code(), e.to_string(), e.status_code()),
    }
}

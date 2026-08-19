//! 场景编排处理函数（P1-3：/api/scene/*）
//!
//! 场景 = 有序服务组；一键启停按 `server_ids` 顺序/逆序操作各服务。

use axum::extract::{Json, State};
use axum::http::HeaderMap;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::backend::api::handlers::{audit_log, err, ok, ok_msg, ok_msg_only, Resp};
use crate::backend::state::AppState;
use crate::backend::types::*;

use uuid::Uuid;

#[derive(Deserialize, utoipa::ToSchema)]
pub(crate) struct SceneId {
    id: String,
}

/// 场景摘要（审计 detail 用）
fn scene_summary(c: &SceneConfig) -> Value {
    json!({
        "name": c.name,
        "serverCount": c.server_ids.len(),
    })
}

/// 场景列表
#[utoipa::path(
    get,
    path = "/api/scene/list",
    responses((status = 200, description = "场景配置列表"))
)]
pub async fn scene_list(State(b): State<AppState>) -> Resp {
    let scenes = b.config.get_scenes();
    ok(serde_json::to_value(scenes).unwrap_or(Value::Null))
}

/// 新增场景（自动生成 id / 时间戳并持久化）
#[utoipa::path(
    post,
    path = "/api/scene/add",
    request_body = SceneConfig,
    responses((status = 200, description = "添加成功，返回新场景"))
)]
pub async fn scene_add(
    State(b): State<AppState>,
    headers: HeaderMap,
    Json(mut body): Json<SceneConfig>,
) -> Resp {
    if body.id.is_empty() {
        body.id = Uuid::new_v4().to_string();
    }
    let now = now_rfc3339();
    if body.created_at.is_empty() {
        body.created_at = now.clone();
    }
    body.updated_at = now;
    let mut scenes = b.config.get_scenes();
    scenes.push(body.clone());
    b.config.save_scenes(scenes);
    audit_log(
        &b,
        &headers,
        "scene_add",
        "scene",
        Some(body.id.clone()),
        scene_summary(&body),
        true,
    )
    .await;
    ok_msg(
        serde_json::to_value(body).unwrap_or(Value::Null),
        "场景添加成功",
    )
}

/// 更新场景
#[utoipa::path(
    post,
    path = "/api/scene/update",
    request_body = SceneConfig,
    responses((status = 200, description = "更新成功"))
)]
pub async fn scene_update(
    State(b): State<AppState>,
    headers: HeaderMap,
    Json(mut body): Json<SceneConfig>,
) -> Resp {
    body.updated_at = now_rfc3339();
    let mut scenes = b.config.get_scenes();
    match scenes.iter().position(|s| s.id == body.id) {
        Some(i) => scenes[i] = body.clone(),
        None => {
            audit_log(
                &b,
                &headers,
                "scene_update",
                "scene",
                Some(body.id.clone()),
                json!({ "error": "SCENE_NOT_FOUND" }),
                false,
            )
            .await;
            return err("SCENE_NOT_FOUND", "场景不存在".into(), 404);
        }
    }
    b.config.save_scenes(scenes);
    audit_log(
        &b,
        &headers,
        "scene_update",
        "scene",
        Some(body.id.clone()),
        scene_summary(&body),
        true,
    )
    .await;
    ok_msg_only("场景更新成功")
}

/// 删除场景（仅移除编排，不停服务）
#[utoipa::path(
    post,
    path = "/api/scene/remove",
    request_body = SceneId,
    responses((status = 200, description = "删除成功"))
)]
pub async fn scene_remove(
    State(b): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<SceneId>,
) -> Resp {
    let scenes = b
        .config
        .get_scenes()
        .into_iter()
        .filter(|s| s.id != body.id)
        .collect();
    b.config.save_scenes(scenes);
    audit_log(
        &b,
        &headers,
        "scene_remove",
        "scene",
        Some(body.id.clone()),
        json!({}),
        true,
    )
    .await;
    ok_msg_only("场景删除成功")
}

/// 一键启动场景：按 server_ids 顺序启动，返回逐服务结果
#[utoipa::path(
    post,
    path = "/api/scene/start",
    request_body = SceneId,
    responses((status = 200, description = "启动结果（逐服务）"))
)]
pub async fn scene_start(
    State(b): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<SceneId>,
) -> Resp {
    let Some(scene) = b.config.get_scene_by_id(&body.id) else {
        audit_log(
            &b,
            &headers,
            "scene_start",
            "scene",
            Some(body.id.clone()),
            json!({ "error": "SCENE_NOT_FOUND" }),
            false,
        )
        .await;
        return err("SCENE_NOT_FOUND", "场景不存在".into(), 404);
    };
    let results = b.services.start_scene(&scene).await;
    let failed = results.iter().filter(|r| !r.success).count();
    audit_log(
        &b,
        &headers,
        "scene_start",
        "scene",
        Some(body.id.clone()),
        json!({
            "name": scene.name,
            "serverCount": scene.server_ids.len(),
            "failed": failed,
        }),
        failed == 0,
    )
    .await;
    ok(serde_json::to_value(results).unwrap_or(Value::Null))
}

/// 一键停止场景：逆序停止，返回停止数
#[utoipa::path(
    post,
    path = "/api/scene/stop",
    request_body = SceneId,
    responses((status = 200, description = "停止成功"))
)]
pub async fn scene_stop(
    State(b): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<SceneId>,
) -> Resp {
    let Some(scene) = b.config.get_scene_by_id(&body.id) else {
        audit_log(
            &b,
            &headers,
            "scene_stop",
            "scene",
            Some(body.id.clone()),
            json!({ "error": "SCENE_NOT_FOUND" }),
            false,
        )
        .await;
        return err("SCENE_NOT_FOUND", "场景不存在".into(), 404);
    };
    let stopped = b.services.stop_scene(&scene).await;
    audit_log(
        &b,
        &headers,
        "scene_stop",
        "scene",
        Some(body.id.clone()),
        json!({
            "name": scene.name,
            "stopped": stopped,
        }),
        true,
    )
    .await;
    ok(json!({ "stopped": stopped }))
}

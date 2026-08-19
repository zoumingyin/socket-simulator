//! 系统设置处理函数（/api/settings）

use axum::extract::{Json, State};
use axum::http::HeaderMap;
use serde::Deserialize;
use serde_json::json;

use crate::backend::api::handlers::{audit_log, ok, ok_msg_only, Resp};
use crate::backend::state::AppState;
use crate::backend::types::*;

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SettingsBody {
    system_settings: Option<SystemSettings>,
    window_config: Option<WindowConfig>,
}

#[utoipa::path(
    get,
    path = "/api/settings",
    responses((status = 200, description = "OK"))
)]
pub async fn get_settings(State(b): State<AppState>) -> Resp {
    let settings = b.config.get_system_settings();
    let window = b.config.get_window_config();
    ok(serde_json::json!({ "systemSettings": settings, "windowConfig": window }))
}

#[utoipa::path(
    post,
    path = "/api/settings",
    request_body = SettingsBody,
    responses((status = 200, description = "OK"))
)]
pub async fn save_settings(
    State(b): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<SettingsBody>,
) -> Resp {
    let has_sys = body.system_settings.is_some();
    let has_win = body.window_config.is_some();
    if let Some(s) = body.system_settings {
        b.config.save_system_settings(s.clone());
        // F-3: 保存设置后用最新保留天数触发一次清理
        b.logs.cleanup_old(s.log_retention_days);
    }
    if let Some(w) = body.window_config {
        b.config.save_window_config(w);
    }
    audit_log(
        &b,
        &headers,
        "settings_update",
        "config",
        None,
        json!({
            "systemSettings": has_sys,
            "windowConfig": has_win,
        }),
        true,
    )
    .await;
    ok_msg_only("设置已保存")
}

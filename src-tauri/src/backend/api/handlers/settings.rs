//! 系统设置处理函数（/api/settings）

use axum::extract::{Json, State};
use serde::Deserialize;

use crate::backend::api::handlers::{ok, ok_msg_only, Resp};
use crate::backend::state::AppState;
use crate::backend::types::*;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SettingsBody {
    system_settings: Option<SystemSettings>,
    window_config: Option<WindowConfig>,
}

pub async fn get_settings(State(b): State<AppState>) -> Resp {
    let settings = b.config.get_system_settings();
    let window = b.config.get_window_config();
    ok(serde_json::json!({ "systemSettings": settings, "windowConfig": window }))
}

pub async fn save_settings(State(b): State<AppState>, Json(body): Json<SettingsBody>) -> Resp {
    if let Some(s) = body.system_settings {
        b.config.save_system_settings(s);
    }
    if let Some(w) = body.window_config {
        b.config.save_window_config(w);
    }
    ok_msg_only("设置已保存")
}

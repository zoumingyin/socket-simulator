//! 配置导入/导出处理函数（/api/export、/api/import）

use axum::extract::{Json, State};
use serde_json::Value;

use crate::backend::api::handlers::{ok, ok_msg_only, Resp};
use crate::backend::state::AppState;
use crate::backend::types::*;

pub async fn export_config(State(b): State<AppState>) -> Resp {
    ok(serde_json::to_value(b.config.export_all()).unwrap_or(Value::Null))
}

pub async fn import_config(State(b): State<AppState>, Json(body): Json<PersistedConfig>) -> Resp {
    // 1. 整体替换持久化配置（旧 config 的 templates 等已移除字段由 serde 容忍未知字段自动忽略 → 自动升级）
    b.config.import_all(body.clone());
    b.events.load_events(body.events.clone());

    // 2. 决策门 3 = A：全量重启受影响服务，确保运行时与导入后的配置/端口立即一致
    //    顺序：停全部 → 按导入配置全量启动（与 app 启动契约一致）→ 恢复独立 Mock 服务
    let _ = b.services.stop_all().await;
    let _ = b.services.start_all().await;
    let sys = b.config.get_system_settings();
    b.mock.restore(&b.config, &sys).await;

    ok_msg_only("导入成功")
}

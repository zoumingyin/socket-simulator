//! 应用状态别名与 Backend 公开字段定义
//!
//! 所有后台任务与 axum handlers 共享同一个 `Arc<Backend>` 作为 AppState。

use std::sync::Arc;

use crate::backend::app::Backend;

/// axum 注入用的 AppState 别名
pub type AppState = Arc<Backend>;

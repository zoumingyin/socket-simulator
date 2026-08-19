//! socket-service-manager 后端（Rust 重写）
//!
//! 该模块整体作为 Tauri 命令/后台任务运行在 `src-tauri` 内，
//! 对外提供：REST API（端口 3080）与纯 WebSocket 管理通道（`/admin/ws`）。

pub mod api;
pub mod app;
pub mod constants;
pub mod error;
pub mod eventbus;
pub mod frontend;
pub mod managers;
pub mod mock;
pub mod net;
pub mod repository;
pub mod auth;
pub mod state;
pub mod transport;
pub mod types;
pub mod ws;

pub use app::run;

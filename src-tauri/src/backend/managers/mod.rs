//! 业务管理器模块

pub mod client_manager;
pub mod config_manager;
pub mod event_manager;
pub mod log_manager;
pub mod service_manager;

#[cfg(test)]
mod service_lifecycle_tests;

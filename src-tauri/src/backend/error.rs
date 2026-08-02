//! 后端统一错误类型与错误码枚举
//!
//! 错误码字符串值与现网 Node 版保持一致，便于前端按 `errorCode` 处理。

use thiserror::Error;

/// 后端统一错误
#[derive(Debug, Error, Clone)]
pub enum BackendError {
    #[error("server not found")]
    ServerNotFound,

    #[error("transport not found")]
    TransportNotFound,

    #[error("internal error: {0}")]
    Internal(String),

    #[error("config error: {0}")]
    Config(String),

    #[error("io error: {0}")]
    Io(String),
}

impl BackendError {
    /// 返回与现网一致的错误码字符串
    pub fn error_code(&self) -> &'static str {
        match self {
            BackendError::ServerNotFound => "SERVER_NOT_FOUND",
            BackendError::TransportNotFound => "TRANSPORT_NOT_FOUND",
            BackendError::Internal(_) => "INTERNAL_ERROR",
            BackendError::Config(_) => "CONFIG_ERROR",
            BackendError::Io(_) => "INTERNAL_ERROR",
        }
    }

    /// 返回对应的 HTTP 状态码
    pub fn status_code(&self) -> u16 {
        match self {
            BackendError::ServerNotFound => 404,
            BackendError::TransportNotFound => 400,
            BackendError::Config(_) => 400,
            BackendError::Internal(_) | BackendError::Io(_) => 500,
        }
    }
}

impl From<std::io::Error> for BackendError {
    fn from(e: std::io::Error) -> Self {
        BackendError::Io(e.to_string())
    }
}

impl From<serde_json::Error> for BackendError {
    fn from(e: serde_json::Error) -> Self {
        BackendError::Config(e.to_string())
    }
}

impl From<anyhow::Error> for BackendError {
    fn from(e: anyhow::Error) -> Self {
        BackendError::Internal(e.to_string())
    }
}

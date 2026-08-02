//! 后端核心数据结构（与 TS 类型契约 1:1 对应）
//!
//! 字段名/类型与现网 `backend/src/types/index.ts` 完全一致，保证 config.json 与
//! REST 响应向前兼容。所有序列化结构体使用 camelCase 命名（与前端一致）。

use serde::{Deserialize, Serialize};

// ======================== 服务配置 ========================

/// 协议类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ProtocolType {
    #[default]
    #[serde(rename = "websocket")]
    Websocket,
    #[serde(rename = "socket.io")]
    SocketIo,
}

/// 日志等级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LogLevel {
    #[default]
    #[serde(rename = "DEBUG")]
    Debug,
    #[serde(rename = "INFO")]
    Info,
    #[serde(rename = "WARN")]
    Warn,
    #[serde(rename = "ERROR")]
    Error,
}

impl LogLevel {
    /// 返回与 serde rename 一致的字符串（用于日志过滤比较）
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
        }
    }
}

/// 服务运行状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ServerStatus {
    #[default]
    #[serde(rename = "stopped")]
    Stopped,
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "error")]
    Error,
    #[serde(rename = "starting")]
    Starting,
    #[serde(rename = "stopping")]
    Stopping,
}

/// 服务配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ServerConfig {
    pub id: String,
    pub name: String,
    pub description: String,
    pub ip: String,
    pub port: u32,
    pub protocol: ProtocolType,
    pub auto_start: bool,
    pub log_level: LogLevel,
    pub wss_enabled: bool,
    pub cert_path: Option<String>,
    pub key_path: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            description: String::new(),
            ip: "0.0.0.0".to_string(),
            port: 0,
            protocol: ProtocolType::Websocket,
            auto_start: false,
            log_level: LogLevel::Info,
            wss_enabled: false,
            cert_path: None,
            key_path: None,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }
}

/// 服务运行时状态
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ServerRuntime {
    pub id: String,
    #[serde(default)]
    pub status: ServerStatus,
    pub started_at: Option<String>,
    pub stopped_at: Option<String>,
    pub error: Option<String>,
    #[serde(default)]
    pub client_count: usize,
    #[serde(default)]
    pub total_connections: u64,
    #[serde(default)]
    pub reconnect_count: u64,
    #[serde(default)]
    pub sent_messages: u64,
    #[serde(default)]
    pub received_messages: u64,
    #[serde(default)]
    pub sent_bytes: u64,
    #[serde(default)]
    pub received_bytes: u64,
}

// ======================== 事件配置 ========================

/// 事件运行状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum EventStatus {
    #[default]
    #[serde(rename = "enabled")]
    Enabled,
    #[serde(rename = "disabled")]
    Disabled,
}

/// 事件配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EventConfig {
    pub id: String,
    pub server_id: String,
    pub name: String,
    #[serde(default)]
    pub is_default: bool,
    #[serde(default)]
    pub status: EventStatus,
    pub description: Option<String>,
    pub handler: Option<String>,
    pub default_message: Option<String>,
    #[serde(default)]
    pub polling_enabled: bool,
    #[serde(default)]
    pub polling_interval: Option<u64>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

// ======================== 客户端管理 ========================

/// 客户端连接状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ClientStatus {
    #[default]
    #[serde(rename = "connected")]
    Connected,
    #[serde(rename = "disconnected")]
    Disconnected,
}

/// 客户端分组类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ClientGroupType {
    #[default]
    #[serde(rename = "custom")]
    Custom,
    #[serde(rename = "device")]
    Device,
    #[serde(rename = "user")]
    User,
    #[serde(rename = "webpage")]
    Webpage,
}

/// 客户端信息
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    pub id: String,
    pub server_id: String,
    pub socket_id: String,
    pub ip_address: String,
    #[serde(default)]
    pub connected_at: String,
    #[serde(default)]
    pub last_activity_at: String,
    #[serde(default)]
    pub protocol: ProtocolType,
    #[serde(default)]
    pub status: ClientStatus,
    pub group: Option<ClientGroupType>,
    pub group_name: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// 客户端分组
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ClientGroup {
    pub id: String,
    pub name: String,
    #[serde(rename = "type", default)]
    pub group_type: ClientGroupType,
    #[serde(default)]
    pub client_ids: Vec<String>,
    #[serde(default)]
    pub created_at: String,
}

// ======================== 消息中心 ========================

/// 消息类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MessageType {
    #[default]
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "json")]
    Json,
}

/// 消息目标类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MessageTargetType {
    #[default]
    #[serde(rename = "broadcast")]
    Broadcast,
    #[serde(rename = "client")]
    Client,
    #[serde(rename = "group")]
    Group,
}

/// 发送消息请求
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageRequest {
    pub server_id: String,
    #[serde(default)]
    pub target_type: MessageTargetType,
    pub target_id: Option<String>,
    pub event: String,
    #[serde(default)]
    pub message_type: MessageType,
    #[serde(default)]
    pub content: String,
    pub metadata: Option<serde_json::Value>,
}

/// 消息模板
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MessageTemplate {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub event: String,
    #[serde(default)]
    pub message_type: MessageType,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub is_default: bool,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

// ======================== 日志系统 ========================

/// 日志条目
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    #[serde(default)]
    pub id: String,
    pub server_id: Option<String>,
    #[serde(default)]
    pub level: LogLevel,
    #[serde(default)]
    pub event: String,
    #[serde(default)]
    pub message: String,
    pub client_id: Option<String>,
    #[serde(default)]
    pub timestamp: String,
    pub metadata: Option<serde_json::Value>,
}

/// 日志过滤条件
#[derive(Debug, Clone, Default, Deserialize)]
pub struct LogFilter {
    pub server_id: Option<String>,
    pub level: Option<LogLevel>,
    pub event: Option<String>,
    pub client_id: Option<String>,
    pub keyword: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
}

// ======================== 统计面板 ========================

/// 实时统计（P2-2 占位类型，本期不聚合）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ServerStats {
    pub server_id: String,
    pub online_clients: u64,
    pub total_connections: u64,
    pub reconnect_count: u64,
    pub sent_messages: u64,
    pub received_messages: u64,
    pub sent_bytes: u64,
    pub received_bytes: u64,
    pub total_bytes: u64,
    pub send_rate: f64,
    pub receive_rate: f64,
    pub uptime: u64,
}

/// 心跳配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HeartbeatConfig {
    pub enabled: bool,
    pub ping_interval: u64,
    pub pong_timeout: u64,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            ping_interval: 30_000,
            pong_timeout: 90_000,
        }
    }
}

// ======================== 安全功能 ========================

/// IP 黑名单/白名单
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IpAccessList {
    #[serde(default)]
    pub whitelist: Vec<String>,
    #[serde(default)]
    pub blacklist: Vec<String>,
}

/// WSS 配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WssConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub cert_path: String,
    #[serde(default)]
    pub key_path: String,
}

// ======================== 系统配置 ========================

/// 系统设置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SystemSettings {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub heartbeat: HeartbeatConfig,
    #[serde(default)]
    pub wss: WssConfig,
    #[serde(default)]
    pub ip_access: IpAccessList,
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default)]
    pub start_minimized: bool,
    #[serde(default)]
    pub log_retention_days: u64,
    #[serde(default)]
    pub max_connections_per_server: u64,
    #[serde(default)]
    pub updated_at: String,
}

/// 窗口配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WindowConfig {
    pub width: u32,
    pub height: u32,
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub maximized: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 800,
            x: None,
            y: None,
            maximized: false,
        }
    }
}

// ======================== 压力测试（P2-1 仅占位类型） ========================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PressureTestConfig {
    pub server_id: String,
    pub concurrent_connections: u32,
    pub message_interval: u32,
    pub message_count: u32,
    pub message_size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PressureTestResult {
    pub qps: f64,
    pub tps: f64,
    pub avg_latency: f64,
    pub p95_latency: f64,
    pub p99_latency: f64,
    pub failure_rate: f64,
    pub total_messages: u64,
    pub successful_messages: u64,
    pub failed_messages: u64,
    pub duration: f64,
}

// ======================== REST API ========================

/// API 标准响应（与现网 `{ success, data?, errorCode?, error?, message?, timestamp }` 对齐）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub timestamp: String,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T, message: Option<String>) -> Self {
        Self {
            success: true,
            data: Some(data),
            error_code: None,
            error: None,
            message,
            timestamp: now_rfc3339(),
        }
    }

    pub fn error(error_code: &str, error: String, message: Option<String>) -> ApiResponse<T> {
        ApiResponse {
            success: false,
            data: None,
            error_code: Some(error_code.to_string()),
            error: Some(error),
            message,
            timestamp: now_rfc3339(),
        }
    }
}

impl<T: Default> Default for ApiResponse<T> {
    fn default() -> Self {
        Self {
            success: false,
            data: None,
            error_code: None,
            error: None,
            message: None,
            timestamp: now_rfc3339(),
        }
    }
}

// ======================== 传输层抽象（文档化契约） ========================

/// WS 消息帧：`{ "event": string, "data": object }`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsFrame {
    pub event: String,
    pub data: serde_json::Value,
}

// ======================== 配置持久化 ========================

/// 持久化配置集合（单 config.json）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedConfig {
    #[serde(default)]
    pub servers: Vec<ServerConfig>,
    #[serde(default)]
    pub events: Vec<EventConfig>,
    #[serde(default)]
    pub templates: Vec<MessageTemplate>,
    #[serde(default)]
    pub system_settings: SystemSettings,
    #[serde(default)]
    pub window_config: WindowConfig,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default)]
    pub exported_at: String,
}

fn default_version() -> String {
    "1.0.0".to_string()
}

impl Default for PersistedConfig {
    fn default() -> Self {
        Self {
            servers: Vec::new(),
            events: Vec::new(),
            templates: Vec::new(),
            system_settings: SystemSettings::default(),
            window_config: WindowConfig::default(),
            version: default_version(),
            exported_at: String::new(),
        }
    }
}

// ======================== 工具函数 ========================

/// 生成 RFC3339 时间戳（UTC）
pub fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// 生成本地时区日期字符串 `YYYY-MM-DD`（日志按日文件名使用）
pub fn local_date_string() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

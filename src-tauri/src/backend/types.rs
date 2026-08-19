//! 后端核心数据结构（与 TS 类型契约 1:1 对应）
//!
//! 字段名/类型与现网 `backend/src/types/index.ts` 完全一致，保证 config.json 与
//! REST 响应向前兼容。所有序列化结构体使用 camelCase 命名（与前端一致）。

use serde::{Deserialize, Serialize};

// ======================== 服务配置 ========================

/// 协议类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, specta::Type, utoipa::ToSchema)]
pub enum ProtocolType {
    #[default]
    #[serde(rename = "websocket")]
    Websocket,
    #[serde(rename = "socket.io")]
    SocketIo,
    #[serde(rename = "http")]
    Http,
}

/// HTTP 方法（受管 HTTP 服务的自定义路由可指定）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default, specta::Type, utoipa::ToSchema)]
pub enum HttpMethod {
    #[default]
    #[serde(rename = "GET")]
    Get,
    #[serde(rename = "POST")]
    Post,
    #[serde(rename = "PUT")]
    Put,
    #[serde(rename = "DELETE")]
    Delete,
    #[serde(rename = "PATCH")]
    Patch,
    #[serde(rename = "HEAD")]
    Head,
    #[serde(rename = "OPTIONS")]
    Options,
    #[serde(rename = "ANY")]
    Any,
}

impl HttpMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Delete => "DELETE",
            HttpMethod::Patch => "PATCH",
            HttpMethod::Head => "HEAD",
            HttpMethod::Options => "OPTIONS",
            HttpMethod::Any => "ANY",
        }
    }
}

/// HTTP 路由类型
/// - Inbound：收消息（body 为 JSON），映射到 on_message
/// - Stream：SSE 长连接，server→client 单向推送
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, specta::Type, utoipa::ToSchema)]
pub enum HttpRouteType {
    #[default]
    #[serde(rename = "inbound")]
    Inbound,
    #[serde(rename = "stream")]
    Stream,
}

/// HTTP 自定义路由配置（每个受管 HTTP 服务可配多条）
#[derive(Debug, Clone, Serialize, Deserialize, Default, specta::Type, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HttpRouteConfig {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub method: HttpMethod,
    /// 路径，支持 `{event}` 占位符（如 `/{event}`、`/order/{event}`）
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub route_type: HttpRouteType,
    /// 固定事件名；若 None 且路径含 `{event}` 则取路径段；否则取末段
    #[serde(default)]
    pub event: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// 日志等级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, specta::Type, utoipa::ToSchema)]
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

/// 头部/查询匹配条件
#[derive(Debug, Clone, Serialize, Deserialize, Default, specta::Type, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MockMatchCondition {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub value: String,
    /// 匹配方式：exact / contains / regex / exists
    #[serde(default = "default_match_kind")]
    pub match_kind: String,
    #[serde(default)]
    pub enabled: bool,
}

fn default_match_kind() -> String {
    "exact".to_string()
}

fn default_true() -> bool {
    true
}

fn default_response_status() -> u16 {
    200
}

fn default_response_body() -> String {
    "{\"message\":\"ok\"}".to_string()
}

/// Mock 模拟规则
#[derive(Debug, Clone, Serialize, Deserialize, Default, specta::Type, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MockRule {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    /// HTTP 方法（含 ANY）
    #[serde(default)]
    pub method: HttpMethod,
    /// 路径模式：精确 `/users`、前缀 `/users/*`、参数 `/users/:id`
    #[serde(default)]
    pub path_pattern: String,
    #[serde(default)]
    pub match_headers: Vec<MockMatchCondition>,
    #[serde(default)]
    pub match_query: Vec<MockMatchCondition>,
    /// 请求体匹配（包含子串；空表示不匹配 body）
    #[serde(default)]
    pub match_body: Option<String>,
    #[serde(default = "default_response_status")]
    pub response_status_code: u16,
    #[serde(default)]
    pub response_headers: Vec<MockMatchCondition>,
    #[serde(default = "default_response_body")]
    pub response_body: String,
    #[serde(default)]
    pub response_delay_ms: u32,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub description: Option<String>,
}

/// Mock 服务配置
#[derive(Debug, Clone, Serialize, Deserialize, Default, specta::Type, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MockServiceConfig {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// 挂载路径（如 `/api/gaop/v1`）；主端口模式下作为前缀；自定义端口模式下作为该端口的根路径
    #[serde(default)]
    pub base_path: String,
    /// 自定义端口；None = 挂载到主端口 3080
    #[serde(default)]
    pub custom_port: Option<u16>,
    /// 未匹配规则时返回的状态码
    #[serde(default = "default_response_status")]
    pub default_status_code: u16,
    /// 未匹配规则时返回的响应体
    #[serde(default = "default_response_body")]
    pub default_response_body: String,
    #[serde(default)]
    pub default_delay_ms: u32,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub rules: Vec<MockRule>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

/// 服务运行状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, specta::Type, utoipa::ToSchema)]
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
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, utoipa::ToSchema)]
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
    /// HTTP 协议自定义路由（仅 protocol=http 生效；为空则用内置默认：
    /// `POST /{event}` 收消息 + `GET /stream` SSE 推送）
    #[serde(default)]
    pub http_routes: Vec<HttpRouteConfig>,
    // ---------- 统一路由模式：Mock HTTP + Socket 共端口 ----------
    /// 是否在同一端口上同时提供 Mock HTTP 响应（统一路由模式）。
    /// 启用后，非 WebSocket 升级的 HTTP 请求将由 Mock 引擎处理（规则匹配 → 模拟响应）。
    /// WebSocket 升级请求（含 `Upgrade: websocket` 头）仍由 Socket 传输层处理。
    /// 两者在同一路由 fallback handler 中通过请求类型自动区分，互不干扰。
    #[serde(default)]
    pub mock_enabled: bool,
    /// Mock 规则列表（mock_enabled=true 时生效）；按顺序匹配，首个命中即返回
    #[serde(default)]
    pub mock_rules: Vec<MockRule>,
    /// Mock 默认状态码（未匹配规则时返回）
    #[serde(default = "default_response_status")]
    pub mock_default_status_code: u16,
    /// Mock 默认响应体（未匹配规则时返回）
    #[serde(default = "default_response_body")]
    pub mock_default_response_body: String,
    /// Mock 默认延迟（ms，未匹配规则时）
    #[serde(default)]
    pub mock_default_delay_ms: u32,
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
            http_routes: Vec::new(),
            mock_enabled: false,
            mock_rules: Vec::new(),
            mock_default_status_code: 200,
            mock_default_response_body: "{\"message\":\"ok\"}".to_string(),
            mock_default_delay_ms: 0,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }
}

/// 服务运行时状态
#[derive(Debug, Clone, Serialize, Deserialize, Default, specta::Type, utoipa::ToSchema)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, specta::Type, utoipa::ToSchema)]
pub enum EventStatus {
    #[default]
    #[serde(rename = "enabled")]
    Enabled,
    #[serde(rename = "disabled")]
    Disabled,
}

/// 事件配置
#[derive(Debug, Clone, Serialize, Deserialize, Default, specta::Type, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EventConfig {
    #[serde(default)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, specta::Type, utoipa::ToSchema)]
pub enum ClientStatus {
    #[default]
    #[serde(rename = "connected")]
    Connected,
    #[serde(rename = "disconnected")]
    Disconnected,
}

/// 客户端分组类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, specta::Type, utoipa::ToSchema)]
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
#[derive(Debug, Clone, Serialize, Deserialize, Default, specta::Type, utoipa::ToSchema)]
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

// ======================== 日志系统 ========================

/// 日志条目
#[derive(Debug, Clone, Serialize, Deserialize, Default, specta::Type, utoipa::ToSchema)]
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
#[derive(Debug, Clone, Default, Deserialize, specta::Type, utoipa::ToSchema)]
pub struct LogFilter {
    pub server_id: Option<String>,
    pub level: Option<LogLevel>,
    pub keyword: Option<String>,
}

// ======================== 统计面板 ========================

/// 心跳配置
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, utoipa::ToSchema)]
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
#[derive(Debug, Clone, Serialize, Deserialize, Default, specta::Type, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct IpAccessList {
    #[serde(default)]
    pub whitelist: Vec<String>,
    #[serde(default)]
    pub blacklist: Vec<String>,
}

/// WSS 配置
#[derive(Debug, Clone, Serialize, Deserialize, Default, specta::Type, utoipa::ToSchema)]
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
#[derive(Debug, Clone, Serialize, Deserialize, Default, specta::Type, utoipa::ToSchema)]
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
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, utoipa::ToSchema)]
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

// ======================== REST API ========================

/// API 标准响应（与现网 `{ success, data?, errorCode?, error?, message?, timestamp }` 对齐）
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, utoipa::ToSchema)]
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

// ======================== 场景编排（P1-3） ========================

/// 场景配置（多服务编排：有序服务组，一键启停）
#[derive(Debug, Clone, Serialize, Deserialize, Default, specta::Type, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SceneConfig {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    /// 服务 ID 有序列表（启动按此顺序、停止逆序）
    #[serde(default)]
    pub server_ids: Vec<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

/// 场景内单个服务的启停结果
#[derive(Debug, Clone, Serialize, specta::Type, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SceneServerResult {
    pub server_id: String,
    pub success: bool,
    pub error: Option<String>,
}

// ======================== 传输层抽象（文档化契约） ========================

/// WS 消息帧：`{ "event": string, "data": object }`
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, utoipa::ToSchema)]
pub struct WsFrame {
    pub event: String,
    pub data: serde_json::Value,
}

// ======================== 配置持久化 ========================

/// 持久化配置集合（单 config.json）
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PersistedConfig {
    #[serde(default)]
    pub servers: Vec<ServerConfig>,
    #[serde(default)]
    pub events: Vec<EventConfig>,
    #[serde(default)]
    pub mock_services: Vec<MockServiceConfig>,
    #[serde(default)]
    pub scenes: Vec<SceneConfig>,
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
    "1.1.0".to_string()
}

impl Default for PersistedConfig {
    fn default() -> Self {
        Self {
            servers: Vec::new(),
            events: Vec::new(),
            mock_services: Vec::new(),
            scenes: Vec::new(),
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

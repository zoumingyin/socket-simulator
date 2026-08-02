//! 全局常量与默认值定义

/// REST + 管理 WS 固定监听地址（loopback）
pub const BIND_HOST: &str = "127.0.0.1";

/// REST API 默认端口（可被环境变量 `API_PORT` 覆盖）
pub const DEFAULT_API_PORT: u16 = 3080;

/// 管理端 WebSocket 路径
pub const ADMIN_WS_PATH: &str = "/admin/ws";

/// 心跳：后端每多少毫秒推送一次 `heartbeat`
pub const HEARTBEAT_INTERVAL_MS: u64 = 10_000;

/// 超过该毫秒数未收到 `heartbeat_ack` 即清理管理连接
pub const HEARTBEAT_TIMEOUT_MS: u64 = 30_000;

/// 日志环形缓冲上限
pub const LOG_MEMORY_LIMIT: usize = 2_000;

/// 初始 `log_batch` 推送条数
pub const LOG_BATCH_INITIAL: usize = 100;

/// 复合 clientId 分隔符（`serverId___socketId`）
pub const CLIENT_ID_SEP: &str = "___";

/// 数值 clamp 取值范围
pub const PING_INTERVAL_MIN: u64 = 5_000;
pub const PING_INTERVAL_MAX: u64 = 300_000;
pub const PONG_TIMEOUT_MIN: u64 = 10_000;
pub const PONG_TIMEOUT_MAX: u64 = 600_000;
pub const LOG_RETENTION_MIN: u64 = 1;
pub const LOG_RETENTION_MAX: u64 = 365;
pub const MAX_CONNECTIONS_MIN: u64 = 1;
pub const MAX_CONNECTIONS_MAX: u64 = 10_000;

/// 事件名（后端 → 前端）
pub const EVT_RUNTIME_UPDATE: &str = "runtime_update";
pub const EVT_CLIENT_UPDATE: &str = "client_update";
pub const EVT_LOG_UPDATE: &str = "log_update";
pub const EVT_LOG_BATCH: &str = "log_batch";
pub const EVT_HEARTBEAT: &str = "heartbeat";

/// 事件名（前端 → 后端）
pub const EVT_HEARTBEAT_ACK: &str = "heartbeat_ack";

/// 端口冲突释放后重试前等待时间
pub const PORT_RELEASE_RETRY_DELAY_MS: u64 = 800;

/// 读取 REST 端口（允许通过环境变量 `API_PORT` 覆盖）
pub fn api_port() -> u16 {
    if let Ok(v) = std::env::var("API_PORT") {
        if let Ok(p) = v.parse::<u16>() {
            return p;
        }
    }
    DEFAULT_API_PORT
}


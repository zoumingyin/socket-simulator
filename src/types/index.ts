/**
 * Socket Service Manager - 完整类型定义
 * 严格模式，禁止 any
 */

// ======================== 服务配置 ========================

/** 协议类型 */
export type ProtocolType = 'websocket' | 'socket.io' | 'http';

/** HTTP 方法（受管 HTTP 服务的自定义路由可指定） */
export type HttpMethod = 'GET' | 'POST' | 'PUT' | 'DELETE' | 'PATCH' | 'HEAD' | 'OPTIONS' | 'ANY';

/** HTTP 路由类型：inbound=收消息 / stream=SSE 推送 */
export type HttpRouteType = 'inbound' | 'stream';

/** HTTP 自定义路由配置 */
export interface HttpRouteConfig {
  id: string;
  method: HttpMethod;
  /** 路径，支持 `{event}` 占位符（如 `/{event}`、`/order/{event}`） */
  path: string;
  routeType: HttpRouteType;
  /** 固定事件名；为空时取路径 {event} 段或末段 */
  event?: string;
  description?: string;
}

/** 日志等级 */
export type LogLevel = 'DEBUG' | 'INFO' | 'WARN' | 'ERROR';

/** 服务运行状态 */
export type ServerStatus = 'stopped' | 'running' | 'error' | 'starting' | 'stopping';

/** 服务配置 */
export interface ServerConfig {
  id: string;
  name: string;
  description: string;
  ip: string;
  port: number;
  protocol: ProtocolType;
  autoStart: boolean;
  logLevel: LogLevel;
  wssEnabled: boolean;
  certPath?: string;
  keyPath?: string;
  /** HTTP 协议自定义路由（仅 protocol=http 生效；为空则用内置默认） */
  httpRoutes?: HttpRouteConfig[];
  // ---------- 统一路由模式：Mock HTTP + Socket 共端口 ----------
  /** 是否在同一端口上同时提供 Mock HTTP 响应（统一路由模式） */
  mockEnabled?: boolean;
  /** Mock 规则列表（mockEnabled=true 时生效） */
  mockRules?: MockRule[];
  /** Mock 默认状态码（未匹配规则时返回） */
  mockDefaultStatusCode?: number;
  /** Mock 默认响应体（未匹配规则时返回） */
  mockDefaultResponseBody?: string;
  /** Mock 默认延迟（ms） */
  mockDefaultDelayMs?: number;
  createdAt: string;
  updatedAt: string;
}

/** 服务运行时状态 */
export interface ServerRuntime {
  id: string;
  status: ServerStatus;
  startedAt?: string;
  stoppedAt?: string;
  error?: string;
  clientCount: number;
  totalConnections: number;
  reconnectCount: number;
  sentMessages: number;
  receivedMessages: number;
  sentBytes: number;
  receivedBytes: number;
}

// ======================== Mock 服务 ========================

/** Mock 头部/查询匹配方式 */
export type MatchKind = 'exact' | 'contains' | 'regex' | 'exists';

/** Mock 头部/查询匹配条件 */
export interface MockMatchCondition {
  key: string;
  value: string;
  matchKind: MatchKind;
  enabled: boolean;
}

/** Mock 模拟规则 */
export interface MockRule {
  id: string;
  name: string;
  method: HttpMethod;
  /** 路径模式：精确 `/users`、前缀 `/users/*`、参数 `/users/:id` */
  pathPattern: string;
  matchHeaders: MockMatchCondition[];
  matchQuery: MockMatchCondition[];
  /** 请求体匹配（包含子串；空表示不匹配 body） */
  matchBody?: string;
  responseStatusCode: number;
  responseHeaders: MockMatchCondition[];
  responseBody: string;
  responseDelayMs: number;
  enabled: boolean;
  description?: string;
}

/** Mock 服务配置 */
export interface MockServiceConfig {
  id: string;
  name: string;
  description: string;
  /** 挂载路径；主端口模式下作为前缀；自定义端口模式下作为该端口的根路径 */
  basePath: string;
  /** 自定义端口；undefined = 挂载到主端口 3080 */
  customPort?: number;
  /** 未匹配规则时返回的状态码 */
  defaultStatusCode: number;
  /** 未匹配规则时返回的响应体 */
  defaultResponseBody: string;
  defaultDelayMs: number;
  enabled: boolean;
  rules: MockRule[];
  createdAt: string;
  updatedAt: string;
}

// ======================== 事件配置 ========================

/** 事件运行状态 */
export type EventStatus = 'enabled' | 'disabled';

/** 默认事件类型 */
export type DefaultEventType = 'connect' | 'disconnect' | 'message';

/** 事件配置 */
export interface EventConfig {
  id: string;
  serverId: string;
  name: string;
  isDefault: boolean;
  status: EventStatus;
  description?: string;
  handler?: string;
  /** 轮询启用后定时广播的消息内容（JSON 或纯文本） */
  defaultMessage?: string;
  /** 是否启用轮询 */
  pollingEnabled?: boolean;
  /** 轮询间隔（秒） */
  pollingInterval?: number;
  createdAt: string;
  updatedAt: string;
}

/** WebSocket 事件消息格式 */
export interface WebSocketEventMessage {
  event: string;
  data: Record<string, unknown>;
}

/** Socket.IO 事件发射 */
export interface SocketIOEventData {
  event: string;
  data: Record<string, unknown>;
}

// ======================== 客户端管理 ========================

/** 客户端连接状态 */
export type ClientStatus = 'connected' | 'disconnected';

/** 客户端分组类型（对应 Rust ClientGroupType） */
export type ClientGroupType = 'custom' | 'device' | 'user' | 'webpage';

/** 客户端信息 */
export interface ClientInfo {
  id: string;
  serverId: string;
  socketId: string;
  ipAddress: string;
  connectedAt: string;
  lastActivityAt: string;
  protocol: ProtocolType;
  status: ClientStatus;
  /** 客户端分组类型（对应 Rust ClientInfo.group） */
  group?: ClientGroupType;
  /** 客户端分组名（对应 Rust ClientInfo.group_name） */
  groupName?: string;
  metadata?: Record<string, unknown>;
}

// ======================== 消息中心 ========================

/** 消息类型 */
export type MessageType = 'text' | 'json';

/** 消息目标类型 */
export type MessageTargetType = 'broadcast' | 'client';

/** 发送消息请求 */
export interface SendMessageRequest {
  serverId: string;
  targetType: MessageTargetType;
  targetId?: string;
  event: string;
  messageType: MessageType;
  content: string;
  metadata?: Record<string, unknown>;
}

/** 本地保存的消息（消息中心 /messages 持久化到 localStorage） */
export interface SavedMessage {
  id: string;
  content: string;
  messageType: MessageType;
  event?: string;
  serverId?: string;
  targetType?: MessageTargetType;
  createdAt: string;
}


// ======================== 日志系统 ========================

/** 日志条目 */
export interface LogEntry {
  id: string;
  serverId?: string;
  level: LogLevel;
  event: string;
  message: string;
  clientId?: string;
  timestamp: string;
  metadata?: Record<string, unknown>;
}

/** 日志过滤条件 */
export interface LogFilter {
  serverId?: string;
  level?: LogLevel;
  event?: string;
  clientId?: string;
  keyword?: string;
  startTime?: string;
  endTime?: string;
}

// ======================== 统计面板 ========================

/** 心跳配置 */
export interface HeartbeatConfig {
  enabled: boolean;
  pingInterval: number;
  pongTimeout: number;
}

// ======================== 安全功能 ========================

/** IP 黑名单/白名单 */
export interface IPAccessList {
  whitelist: string[];
  blacklist: string[];
}

/** WSS 配置 */
export interface WssConfig {
  enabled: boolean;
  certPath: string;
  keyPath: string;
}

// ======================== 系统配置 ========================

/** 系统设置 */
export interface SystemSettings {
  id: string;
  heartbeat: HeartbeatConfig;
  wss: WssConfig;
  ipAccess: IPAccessList;
  autoStart: boolean;
  startMinimized: boolean;
  logRetentionDays: number;
  maxConnectionsPerServer: number;
  updatedAt: string;
}

/** 窗口配置 */
export interface WindowConfig {
  width: number;
  height: number;
  x?: number;
  y?: number;
  maximized: boolean;
}

/** 应用配置 */
export interface AppConfig {
  systemSettings: SystemSettings;
  windowConfig: WindowConfig;
}

// ======================== REST API ========================

/** API 标准响应 */
export interface ApiResponse<T = unknown> {
  success: boolean;
  data?: T;
  error?: string;
  errorCode?: string;
  message?: string;
  timestamp: string;
}

/** 分页请求 */
export interface PaginationRequest {
  page: number;
  pageSize: number;
}

/** 分页响应 */
export interface PaginatedResponse<T> {
  items: T[];
  total: number;
  page: number;
  pageSize: number;
}

// ======================== 配置持久化 ========================

/** 持久化配置集合 */
export interface PersistedConfig {
  servers: ServerConfig[];
  events: EventConfig[];
  /** 独立 Mock 服务列表（对应 Rust PersistedConfig.mock_services） */
  mockServices?: MockServiceConfig[];
  systemSettings: SystemSettings;
  windowConfig: WindowConfig;
  version: string;
  exportedAt: string;
}


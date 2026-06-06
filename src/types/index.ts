/**
 * Socket Service Manager - 完整类型定义
 * 严格模式，禁止 any
 */

// ======================== 服务配置 ========================

/** 协议类型 */
export type ProtocolType = 'websocket' | 'socket.io';

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

/** 实时统计 */
export interface ServerStats {
  serverId: string;
  onlineClients: number;
  totalConnections: number;
  reconnectCount: number;
  sentMessages: number;
  receivedMessages: number;
  sentBytes: number;
  receivedBytes: number;
  totalBytes: number;
  sendRate: number;
  receiveRate: number;
  uptime: number;
}

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

// ======================== 压力测试 ========================

/** 压力测试配置 */
export interface PressureTestConfig {
  serverId: string;
  concurrentConnections: number;
  messageInterval: number;
  messageCount: number;
  messageSize: number;
}

/** 压力测试结果 */
export interface PressureTestResult {
  qps: number;
  tps: number;
  avgLatency: number;
  p95Latency: number;
  p99Latency: number;
  failureRate: number;
  totalMessages: number;
  successfulMessages: number;
  failedMessages: number;
  duration: number;
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

// ======================== MCP 工具 ========================

/** MCP 工具定义 */
export interface McpToolDefinition {
  name: string;
  description: string;
  inputSchema: Record<string, unknown>;
}

/** MCP 调用结果 */
export interface McpToolResult {
  content: Array<{
    type: string;
    text: string;
  }>;
  isError?: boolean;
}

// ======================== 传输层抽象 ========================

/** 传输层事件 */
export interface TransportEvents {
  connect: (client: ClientInfo) => void;
  disconnect: (clientId: string, reason?: string) => void;
  message: (clientId: string, event: string, data: unknown) => void;
  error: (error: Error) => void;
}

/** 传输层接口（协议插件化） */
export interface ITransport {
  readonly protocol: ProtocolType;
  on(event: string, handler: (...args: any[]) => void): this;
  emit(event: string, ...args: any[]): boolean;
  start(): Promise<void>;
  stop(): Promise<void>;
  send(clientId: string, event: string, data: unknown): Promise<void>;
  broadcast(event: string, data: unknown, targetIds?: string[]): Promise<void>;
  disconnectClient(clientId: string): Promise<void>;
  getClients(): ClientInfo[];
  isRunning(): boolean;
}

// ======================== 配置持久化 ========================

/** 持久化配置集合 */
export interface PersistedConfig {
  servers: ServerConfig[];
  events: EventConfig[];
  systemSettings: SystemSettings;
  windowConfig: WindowConfig;
  version: string;
  exportedAt: string;
}

// ======================== 前端状态 ========================

/** 前端全局状态 */
export interface RootState {
  servers: ServerState;
  clients: ClientState;
  events: EventState;
  messages: MessageState;
  logs: LogState;
  settings: SettingsState;
  stats: StatsState;
}

export interface ServerState {
  list: ServerConfig[];
  runtimes: Record<string, ServerRuntime>;
  loading: boolean;
  error?: string;
}

export interface ClientState {
  list: ClientInfo[];
  loading: boolean;
  error?: string;
}

export interface EventState {
  list: EventConfig[];
  loading: boolean;
  error?: string;
}

export interface MessageState {
  sending: boolean;
  error?: string;
}

export interface LogState {
  entries: LogEntry[];
  filter: LogFilter;
  autoScroll: boolean;
  loading: boolean;
}

export interface SettingsState {
  systemSettings: SystemSettings;
  windowConfig: WindowConfig;
  loading: boolean;
}

export interface StatsState {
  stats: Record<string, ServerStats>;
  loading: boolean;
}

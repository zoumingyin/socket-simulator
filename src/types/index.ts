/**
 * Socket Service Manager - 前端类型契约
 *
 * 后端形态的数据类型（ServerConfig / ClientInfo / LogEntry ...）由
 * `./generated` 从 Rust（`src-tauri/src/backend/types.rs`，specta 1:1 导出）
 * 自动生成，单一权威源，禁止在此手工重复声明。
 * 运行：`cd src-tauri && cargo run --bin export_types`
 *
 * 本文件只保留「前端独有、后端无对应」的类型：
 * 消息中心、WS/SIO 事件封装、分页、App 配置、前端扩展版 LogFilter 等。
 *
 * 注意：specta 1.0.5 把 Rust `Option<T>` 导出为 `T | null`（非 `T | undefined`），
 * 前端消费生成类型时按此契约适配（读接口时 `| null` 更准确；构造时显式给 `null`）。
 */

// ======================== 后端类型（自动生成，单一权威源） ========================
export * from './generated';

// 本文件内复用生成类型时显式引入（re-export 不会把名字带进当前模块作用域）
import type { LogLevel, SystemSettings, WindowConfig } from './generated';

// ======================== 前端独有类型 ========================

/** Mock 头部/查询匹配方式（前端语义枚举；生成类型 MockMatchCondition.matchKind 为 string） */
export type MatchKind = 'exact' | 'contains' | 'regex' | 'exists';

/** 默认事件类型 */
export type DefaultEventType = 'connect' | 'disconnect' | 'message';

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

/** 日志过滤条件（前端扩展版：camelCase + 关键字/时间区间，仅前端 UI 使用） */
export interface LogFilter {
  serverId?: string;
  level?: LogLevel;
  event?: string;
  clientId?: string;
  keyword?: string;
  startTime?: string;
  endTime?: string;
}

/** IP 黑名单/白名单（前端语义版；生成类型为 IpAccessList，字段同名不同大小写） */
export interface IPAccessList {
  whitelist: string[];
  blacklist: string[];
}

/** 应用配置 */
export interface AppConfig {
  systemSettings: SystemSettings;
  windowConfig: WindowConfig;
}

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

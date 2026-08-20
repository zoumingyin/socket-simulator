// 本文件由 `cargo run --bin export_types` 自动生成，请勿手工修改。
// 权威源：src-tauri/src/backend/types.rs（specta 1:1 导出）

/**
 * 协议类型
 */
export type ProtocolType = "websocket" | "socket.io" | "http" | "tcp" | "udp" | "mqtt" | "sse"/**
 * HTTP 方法（受管 HTTP 服务的自定义路由可指定）
 */
export type HttpMethod = "GET" | "POST" | "PUT" | "DELETE" | "PATCH" | "HEAD" | "OPTIONS" | "ANY"/**
 * HTTP 路由类型
 * - Inbound：收消息（body 为 JSON），映射到 on_message
 * - Stream：SSE 长连接，server→client 单向推送
 */
export type HttpRouteType = "inbound" | "stream"/**
 * 日志等级
 */
export type LogLevel = "DEBUG" | "INFO" | "WARN" | "ERROR"/**
 * 服务运行状态
 */
export type ServerStatus = "stopped" | "running" | "error" | "starting" | "stopping"/**
 * 事件运行状态
 */
export type EventStatus = "enabled" | "disabled"/**
 * 客户端连接状态
 */
export type ClientStatus = "connected" | "disconnected"/**
 * 客户端分组类型
 */
export type ClientGroupType = "custom" | "device" | "user" | "webpage"/**
 * HTTP 自定义路由配置（每个受管 HTTP 服务可配多条）
 */
export type HttpRouteConfig = { id?: string; method?: HttpMethod; path?: string; routeType?: HttpRouteType; event?: string | null; description?: string | null }/**
 * 头部/查询匹配条件
 */
export type MockMatchCondition = { key?: string; value?: string; matchKind?: string; enabled?: boolean }/**
 * Mock 模拟规则
 */
export type MockRule = { id?: string; name?: string; method?: HttpMethod; pathPattern?: string; matchHeaders?: MockMatchCondition[]; matchQuery?: MockMatchCondition[]; matchBody?: string | null; responseStatusCode?: number; responseHeaders?: MockMatchCondition[]; responseBody?: string; responseDelayMs?: number; enabled?: boolean; description?: string | null }/**
 * Mock 服务配置
 */
export type MockServiceConfig = { id?: string; name?: string; description?: string; basePath?: string; customPort?: number | null; defaultStatusCode?: number; defaultResponseBody?: string; defaultDelayMs?: number; enabled?: boolean; rules?: MockRule[]; createdAt?: string; updatedAt?: string }/**
 * 场景配置（多服务编排：有序服务组，一键启停）
 */
export type SceneConfig = { id?: string; name?: string; description?: string | null; serverIds?: string[]; enabled?: boolean; createdAt?: string; updatedAt?: string }/**
 * 场景内单个服务的启停结果
 */
export type SceneServerResult = { serverId: string; success: boolean; error: string | null }/**
 * 服务配置
 */
export type ServerConfig = { id: string; name: string; description: string; ip: string; port: number; protocol: ProtocolType; autoStart: boolean; logLevel: LogLevel; wssEnabled: boolean; certPath: string | null; keyPath: string | null; httpRoutes?: HttpRouteConfig[]; mockEnabled?: boolean; mockRules?: MockRule[]; mockDefaultStatusCode?: number; mockDefaultResponseBody?: string; mockDefaultDelayMs?: number; createdAt: string; updatedAt: string }/**
 * 服务运行时状态
 */
export type ServerRuntime = { id: string; status?: ServerStatus; startedAt: string | null; stoppedAt: string | null; error: string | null; clientCount?: number; totalConnections?: number; reconnectCount?: number; sentMessages?: number; receivedMessages?: number; sentBytes?: number; receivedBytes?: number }/**
 * 事件配置
 */
export type EventConfig = { id?: string; serverId: string; name: string; isDefault?: boolean; status?: EventStatus; description: string | null; handler: string | null; defaultMessage: string | null; pollingEnabled?: boolean; pollingInterval?: number | null; createdAt?: string; updatedAt?: string }/**
 * 客户端信息
 */
export type ClientInfo = { id: string; serverId: string; socketId: string; ipAddress: string; connectedAt?: string; lastActivityAt?: string; protocol?: ProtocolType; status?: ClientStatus; group: ClientGroupType | null; groupName: string | null; metadata: any | null }/**
 * 日志条目
 */
export type LogEntry = { id?: string; serverId: string | null; level?: LogLevel; event?: string; message?: string; clientId: string | null; timestamp?: string; metadata: any | null }/**
 * 心跳配置
 */
export type HeartbeatConfig = { enabled: boolean; pingInterval: number; pongTimeout: number }/**
 * IP 黑名单/白名单
 */
export type IpAccessList = { whitelist?: string[]; blacklist?: string[] }/**
 * WSS 配置
 */
export type WssConfig = { enabled?: boolean; certPath?: string; keyPath?: string }/**
 * 系统设置
 */
export type SystemSettings = { id?: string; heartbeat?: HeartbeatConfig; wss?: WssConfig; ipAccess?: IpAccessList; autoStart?: boolean; startMinimized?: boolean; logRetentionDays?: number; maxConnectionsPerServer?: number; updatedAt?: string }/**
 * 窗口配置
 */
export type WindowConfig = { width: number; height: number; x: number | null; y: number | null; maximized: boolean }/**
 * WS 消息帧：`{ "event": string, "data": object }`
 */
export type WsFrame = { event: string; data: any }/**
 * 持久化配置集合（单 config.json）
 */
export type PersistedConfig = { servers?: ServerConfig[]; events?: EventConfig[]; mockServices?: MockServiceConfig[]; scenes?: SceneConfig[]; systemSettings?: SystemSettings; windowConfig?: WindowConfig; version?: string; exportedAt?: string }
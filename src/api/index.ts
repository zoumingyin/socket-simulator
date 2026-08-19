/**
 * api/index.ts - 前端 typed API 模块（P3-1）
 *
 * 把所有 REST 端点收敛到一个带类型的命名空间 `api`，消除各 store 中散落的
 * 裸路径字符串（如 apiFetch('/api/server/start')）。每个方法只负责「拼路径 +
 * 序列化 body + 标注返回类型」，错误语义与底层 apiFetch 保持一致：失败时返回
 * `{ success: false, error }`，调用方照常检查 res.success / res.data。
 *
 * 路径前缀说明：client.ts 会自动把非 `/api/` 开头的路径补上 `/api`，因此
 * `/server/list` 与 `/api/server/list` 等价于同一后端路由，本模块沿用各 store
 * 原用的写法以零行为变更。
 */
import { apiFetch } from './client';
import type {
  ServerConfig,
  ServerRuntime,
  EventConfig,
  ClientInfo,
  LogEntry,
  LogFilter,
  SystemSettings,
  WindowConfig,
  SendMessageRequest,
  MockServiceConfig,
  SceneConfig,
  SceneServerResult,
} from '../types/index';

/** 构建查询字符串；跳过空值，返回 "?a=1&b=2" 或 "" */
function buildQuery(params: Record<string, string | undefined>): string {
  const sp = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value != null && value !== '') sp.set(key, value);
  }
  const s = sp.toString();
  return s ? `?${s}` : '';
}

/** 服务管理 */
export const servers = {
  /** 轻量列表（id + name），供下拉/事件关联使用 */
  list: () => apiFetch<ServerConfig[]>('/server/list'),
  /** 完整列表（GET /api/servers） */
  all: () => apiFetch<ServerConfig[]>('/servers'),
  /** 运行时快照（兜底首屏） */
  runtimes: () => apiFetch<Record<string, ServerRuntime>>('/server/runtimes'),
  add: (cfg: Omit<ServerConfig, 'id' | 'createdAt' | 'updatedAt'>) =>
    apiFetch<ServerConfig>('/server/add', { method: 'POST', body: JSON.stringify(cfg) }),
  update: (cfg: ServerConfig) =>
    apiFetch<ServerConfig>('/server/update', { method: 'POST', body: JSON.stringify(cfg) }),
  remove: (id: string) =>
    apiFetch('/server/remove', { method: 'POST', body: JSON.stringify({ id }) }),
  start: (id: string) =>
    apiFetch('/server/start', { method: 'POST', body: JSON.stringify({ id }) }),
  stop: (id: string) =>
    apiFetch('/server/stop', { method: 'POST', body: JSON.stringify({ id }) }),
  restart: (id: string) =>
    apiFetch('/server/restart', { method: 'POST', body: JSON.stringify({ id }) }),
  startAll: () => apiFetch('/server/start-all', { method: 'POST' }),
  stopAll: () => apiFetch('/server/stop-all', { method: 'POST' }),
  restartAll: () => apiFetch('/server/restart-all', { method: 'POST' }),
};

/** 事件管理 */
export const events = {
  list: (serverId?: string) =>
    apiFetch<EventConfig[]>(`/events${buildQuery({ serverId })}`),
  add: (cfg: Omit<EventConfig, 'id' | 'createdAt' | 'updatedAt'>) =>
    apiFetch<EventConfig>('/events/add', { method: 'POST', body: JSON.stringify(cfg) }),
  update: (id: string, patch: Partial<EventConfig>) =>
    apiFetch<EventConfig>('/events/update', { method: 'POST', body: JSON.stringify({ id, ...patch }) }),
  remove: (id: string) =>
    apiFetch('/events/remove', { method: 'POST', body: JSON.stringify({ id }) }),
  toggle: (id: string, status: 'enabled' | 'disabled') =>
    apiFetch('/events/toggle', { method: 'POST', body: JSON.stringify({ id, status }) }),
};

/** 客户端管理 + 消息发送（收敛 send / broadcast 为单一 send 方法） */
export const clients = {
  list: (serverId?: string) =>
    apiFetch<ClientInfo[]>(`/clients${buildQuery({ serverId })}`),
  disconnect: (clientId: string) =>
    apiFetch('/client/disconnect', { method: 'POST', body: JSON.stringify({ clientId }) }),
  /** 单一发送入口：targetType 决定「指定客户端(client)」还是「广播(broadcast)」 */
  send: (req: SendMessageRequest) =>
    apiFetch('/client/send', { method: 'POST', body: JSON.stringify(req) }),
};

/** 日志查看 */
export const logs = {
  list: (filter?: LogFilter) =>
    apiFetch<LogEntry[]>(
      `/logs${buildQuery({
        serverId: filter?.serverId,
        level: filter?.level,
        keyword: filter?.keyword,
      })}`
    ),
  clear: () => apiFetch('/logs/clear', { method: 'POST' }),
};

/** 系统设置 */
export const settings = {
  get: () => apiFetch<{ systemSettings: SystemSettings; windowConfig: WindowConfig }>('/settings'),
  save: (body: { systemSettings?: SystemSettings; windowConfig?: WindowConfig }) =>
    apiFetch('/settings', { method: 'POST', body: JSON.stringify(body) }),
};

/** 配置导入导出（JSON 透传边界，类型宽松） */
export const config = {
  export: () => apiFetch<Record<string, unknown>>('/export'),
  import: (cfg: Record<string, unknown>) =>
    apiFetch('/import', { method: 'POST', body: JSON.stringify(cfg) }),
};

/** Mock 服务（独立 Mock 引擎入口） */
export const mock = {
  list: () => apiFetch<MockServiceConfig[]>('/mock/list'),
  get: (id: string) =>
    apiFetch<MockServiceConfig>('/mock/get', { method: 'POST', body: JSON.stringify({ id }) }),
  add: (cfg: Omit<MockServiceConfig, 'id' | 'createdAt' | 'updatedAt'>) =>
    apiFetch<MockServiceConfig>('/mock/add', { method: 'POST', body: JSON.stringify(cfg) }),
  update: (cfg: MockServiceConfig) =>
    apiFetch<MockServiceConfig>('/mock/update', { method: 'POST', body: JSON.stringify(cfg) }),
  remove: (id: string) =>
    apiFetch('/mock/remove', { method: 'POST', body: JSON.stringify({ id }) }),
  start: (id: string) =>
    apiFetch('/mock/start', { method: 'POST', body: JSON.stringify({ id }) }),
  stop: (id: string) =>
    apiFetch('/mock/stop', { method: 'POST', body: JSON.stringify({ id }) }),
};

/** 场景编排（P1-3） */
export const scenes = {
  list: () => apiFetch<SceneConfig[]>('/scene/list'),
  add: (cfg: Omit<SceneConfig, 'id' | 'createdAt' | 'updatedAt'>) =>
    apiFetch<SceneConfig>('/scene/add', { method: 'POST', body: JSON.stringify(cfg) }),
  update: (cfg: SceneConfig) =>
    apiFetch('/scene/update', { method: 'POST', body: JSON.stringify(cfg) }),
  remove: (id: string) =>
    apiFetch('/scene/remove', { method: 'POST', body: JSON.stringify({ id }) }),
  start: (id: string) =>
    apiFetch<SceneServerResult[]>('/scene/start', { method: 'POST', body: JSON.stringify({ id }) }),
  stop: (id: string) =>
    apiFetch<{ stopped: number }>('/scene/stop', { method: 'POST', body: JSON.stringify({ id }) }),
};

/** 统一入口 */
export const api = { servers, events, clients, logs, settings, config, mock, scenes };
export default api;

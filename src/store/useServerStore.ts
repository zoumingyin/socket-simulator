/**
 * useServerStore - 服务管理 Zustand Store
 * 管理服务配置列表、运行时状态、加载状态
 *
 * 数据获取策略：初始加载使用 HTTP，后续更新全部通过 WebSocket 实时推送
 */
import { create } from 'zustand';
import type { ServerConfig, ServerRuntime } from '../types/index';
import { apiFetch } from '../api/client';
import { adminSocket } from '../socket/AdminSocketManager';

interface ServerState {
  list: ServerConfig[];
  runtimes: Record<string, ServerRuntime>;
  loading: boolean;
  error?: string;

  fetchServers: () => Promise<void>;
  fetchRuntimes: () => Promise<void>;
  addServer: (config: Omit<ServerConfig, 'id' | 'createdAt' | 'updatedAt'>) => Promise<ServerConfig>;
  updateServer: (id: string, patch: Partial<ServerConfig>) => Promise<void>;
  removeServer: (id: string) => Promise<void>;
  startServer: (id: string) => Promise<void>;
  stopServer: (id: string) => Promise<void>;
  restartServer: (id: string) => Promise<void>;
  startAll: () => Promise<void>;
  stopAll: () => Promise<void>;
  restartAll: () => Promise<void>;
  batchStart: (ids: string[]) => Promise<void>;
  batchStop: (ids: string[]) => Promise<void>;
  batchRestart: (ids: string[]) => Promise<void>;
  batchDelete: (ids: string[]) => Promise<void>;
  setRuntime: (id: string, rt: ServerRuntime) => void;
}

// 订阅 runtime_update 事件（store 外部，只注册一次）
let runtimeSubscribed = false;
function subscribeRuntimeUpdates(): void {
  if (runtimeSubscribed) return;
  runtimeSubscribed = true;

  adminSocket.subscribe('runtime_update', (data) => {
    const runtimes = data as Record<string, ServerRuntime>;
    useServerStore.setState({ runtimes });
  });
}

export const useServerStore = create<ServerState>((set, get) => ({
  list: [],
  runtimes: {},
  loading: false,
  error: undefined,

  async fetchServers() {
    set({ loading: true, error: undefined });
    try {
      const res = await apiFetch<ServerConfig[]>('/server/list');
      set({ list: res.data ?? [], loading: false });
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
    }
  },

  async fetchRuntimes() {
    subscribeRuntimeUpdates();
    // HTTP 作为首次加载兜底
    try {
      const res = await apiFetch<Record<string, ServerRuntime>>('/server/runtimes');
      if (res.data) set({ runtimes: res.data });
    } catch {
      // 忽略 HTTP 失败，Socket 推送会更新数据
    }
  },

  async addServer(config) {
    const res = await apiFetch<ServerConfig>('/server/add', {
      method: 'POST',
      body: JSON.stringify(config),
    });
    if (!res.data) throw new Error(res.error ?? '添加失败');
    set((s) => ({ list: [...s.list, res.data!] }));
    return res.data;
  },

  async updateServer(id, patch) {
    const current = get().list.find((srv) => srv.id === id);
    if (!current) throw new Error('服务不存在');
    const next: ServerConfig = { ...current, ...patch, id };
    const res = await apiFetch<ServerConfig>('/server/update', {
      method: 'POST',
      body: JSON.stringify(next),
    });
    const saved = res.data ?? next;
    set((s) => ({
      list: s.list.map((srv) => (srv.id === id ? saved : srv)),
    }));
  },

  async removeServer(id) {
    await apiFetch('/server/remove', {
      method: 'POST',
      body: JSON.stringify({ id }),
    });
    set((s) => ({
      list: s.list.filter((srv) => srv.id !== id),
      runtimes: (() => { const { [id]: _, ...rest } = s.runtimes; return rest; })(),
    }));
  },

  async startServer(id) {
    await apiFetch('/server/start', {
      method: 'POST',
      body: JSON.stringify({ id }),
    });
    // Socket 推送会自动更新 runtimes
  },

  async stopServer(id) {
    await apiFetch(`/server/stop`, {
      method: 'POST',
      body: JSON.stringify({ id }),
    });
  },

  async restartServer(id) {
    await apiFetch(`/server/restart`, {
      method: 'POST',
      body: JSON.stringify({ id }),
    });
  },

  async startAll() {
    await apiFetch(`/server/start-all`, { method: 'POST' });
  },

  async stopAll() {
    await apiFetch(`/server/stop-all`, { method: 'POST' });
  },

  async restartAll() {
    await apiFetch(`/server/restart-all`, { method: 'POST' });
  },

  async batchStart(ids: string[]) {
    await Promise.all(ids.map(id => get().startServer(id)));
  },

  async batchStop(ids: string[]) {
    await Promise.all(ids.map(id => get().stopServer(id)));
  },

  async batchRestart(ids: string[]) {
    await Promise.all(ids.map(id => get().restartServer(id)));
  },

  async batchDelete(ids: string[]) {
    await Promise.all(ids.map(id => get().removeServer(id)));
  },

  setRuntime(id, rt) {
    set((s) => ({ runtimes: { ...s.runtimes, [id]: rt } }));
  },
}));

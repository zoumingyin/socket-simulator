/**
 * useServerStore - 服务管理 Zustand Store
 * 管理服务配置列表、运行时状态、加载状态
 */
import { create } from 'zustand';
import type { ServerConfig, ServerRuntime, ApiResponse } from '../types/index';
import { apiFetch } from '../api/client';
import { nanoid } from 'nanoid';

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
  setRuntime: (id: string, rt: ServerRuntime) => void;
  setRuntimes: (rts: Record<string, ServerRuntime>) => void;
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
    try {
      const res = await apiFetch<Record<string, ServerRuntime>>('/server/runtimes');
      if (res.data) set({ runtimes: res.data });
    } catch {
      // 忽略运行时获取失败
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
    const res = await apiFetch<ServerConfig>('/server/update', {
      method: 'POST',
      body: JSON.stringify({ id, ...patch }),
    });
    if (res.data) {
      set((s) => ({
        list: s.list.map((srv) => (srv.id === id ? res.data! : srv)),
      }));
    }
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
    // 操作后刷新运行时状态
    await get().fetchRuntimes();
  },

  async stopServer(id) {
    await apiFetch(`/server/stop`, {
      method: 'POST',
      body: JSON.stringify({ id }),
    });
    await get().fetchRuntimes();
  },

  async restartServer(id) {
    await apiFetch(`/server/restart`, {
      method: 'POST',
      body: JSON.stringify({ id }),
    });
    await get().fetchRuntimes();
  },

  async startAll() {
    await apiFetch(`/server/start-all`, { method: 'POST' });
    await get().fetchRuntimes();
  },

  async stopAll() {
    await apiFetch(`/server/stop-all`, { method: 'POST' });
    await get().fetchRuntimes();
  },

  async restartAll() {
    await apiFetch(`/server/restart-all`, { method: 'POST' });
    await get().fetchRuntimes();
  },

  setRuntime(id, rt) {
    set((s) => ({ runtimes: { ...s.runtimes, [id]: rt } }));
  },

  setRuntimes(rts) {
    set({ runtimes: rts });
  },
}));

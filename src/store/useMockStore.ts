/**
 * useMockStore - Mock 服务 zustand store
 * 提供 CRUD + 启停；列表通过 admin WS 增量更新（runtime_update）
 */
import { create } from 'zustand';
import type { MockServiceConfig } from '../types/index.js';
import { apiFetch } from '../api/client.js';

interface MockStoreState {
  list: MockServiceConfig[];
  loading: boolean;
  error: string | null;

  fetchList: () => Promise<void>;
  add: (cfg: MockServiceConfig) => Promise<MockServiceConfig>;
  update: (cfg: MockServiceConfig) => Promise<MockServiceConfig>;
  remove: (id: string) => Promise<void>;
  start: (id: string) => Promise<{ port: number }>;
  stop: (id: string) => Promise<void>;

  /** 直接覆盖列表（admin WS 推送时使用） */
  setList: (list: MockServiceConfig[]) => void;
}

export const useMockStore = create<MockStoreState>((set) => ({
  list: [],
  loading: false,
  error: null,

  fetchList: async () => {
    set({ loading: true, error: null });
    try {
      const res = await apiFetch('/mock/list');
      if (res.success && Array.isArray(res.data)) {
        set({ list: res.data as MockServiceConfig[], loading: false });
      } else {
        set({ error: res.error || '加载失败', loading: false });
      }
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
    }
  },

  add: async (cfg) => {
    const res = await apiFetch('/mock/add', { method: 'POST', body: JSON.stringify(cfg) });
    if (!res.success) throw new Error(res.error || '添加失败');
    const created = res.data as MockServiceConfig;
    set((s) => ({ list: [...s.list, created] }));
    return created;
  },

  update: async (cfg) => {
    const res = await apiFetch('/mock/update', { method: 'POST', body: JSON.stringify(cfg) });
    if (!res.success) throw new Error(res.error || '更新失败');
    const updated = res.data as MockServiceConfig;
    set((s) => ({
      list: s.list.map((m) => (m.id === updated.id ? updated : m)),
    }));
    return updated;
  },

  remove: async (id) => {
    const res = await apiFetch('/mock/remove', { method: 'POST', body: JSON.stringify({ id }) });
    if (!res.success) throw new Error(res.error || '删除失败');
    set((s) => ({ list: s.list.filter((m) => m.id !== id) }));
  },

  start: async (id) => {
    const res = await apiFetch('/mock/start', { method: 'POST', body: JSON.stringify({ id }) });
    if (!res.success) throw new Error(res.error || '启动失败');
    return (res.data as { port: number }) || { port: 0 };
  },

  stop: async (id) => {
    const res = await apiFetch('/mock/stop', { method: 'POST', body: JSON.stringify({ id }) });
    if (!res.success) throw new Error(res.error || '停止失败');
  },

  setList: (list) => set({ list }),
}));
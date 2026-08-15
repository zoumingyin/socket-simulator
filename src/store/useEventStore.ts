/**
 * useEventStore - 事件管理 Zustand Store
 */
import { create } from 'zustand';
import type { EventConfig } from '../types/index';
import { api } from '../api';

interface EventState {
  list: EventConfig[];
  servers: { id: string; name: string }[];
  loading: boolean;
  error?: string;

  fetchEvents: (serverId?: string) => Promise<void>;
  fetchServers: () => Promise<void>;
  addEvent: (config: Omit<EventConfig, 'id' | 'createdAt' | 'updatedAt'>) => Promise<EventConfig>;
  updateEvent: (id: string, patch: Partial<EventConfig>) => Promise<void>;
  removeEvent: (id: string) => Promise<void>;
  toggleEvent: (id: string, status: 'enabled' | 'disabled') => Promise<void>;
}

export const useEventStore = create<EventState>((set) => ({
  list: [],
  servers: [],
  loading: false,
  error: undefined,

  async fetchServers() {
    try {
      const res = await api.servers.list();
      set({ servers: res.data ?? [] });
    } catch (e) {
      // 忽略加载失败
    }
  },

  async fetchEvents(serverId) {
    set({ loading: true, error: undefined });
    try {
      const res = await api.events.list(serverId);
      set({ list: res.data ?? [], loading: false });
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
    }
  },

  async addEvent(config) {
    const res = await api.events.add(config);
    if (!res.data) throw new Error(res.error ?? '添加失败');
    set((s) => ({ list: [...s.list, res.data!] }));
    return res.data;
  },

  async updateEvent(id, patch) {
    const res = await api.events.update(id, patch);
    if (res.data) {
      set((s) => ({ list: s.list.map((e) => (e.id === id ? res.data! : e)) }));
    }
  },

  async removeEvent(id) {
    await api.events.remove(id);
    set((s) => ({ list: s.list.filter((e) => e.id !== id) }));
  },

  async toggleEvent(id, status) {
    await api.events.toggle(id, status);
    set((s) => ({
      list: s.list.map((e) => (e.id === id ? { ...e, status } : e)),
    }));
  },
}));

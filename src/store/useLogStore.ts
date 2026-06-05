/**
 * useLogStore - 日志查看器 Zustand Store
 */
import { create } from 'zustand';
import type { LogEntry, LogFilter } from '../types/index';
import { apiFetch } from '../api/client';

interface LogState {
  entries: LogEntry[];
  filter: LogFilter;
  autoScroll: boolean;
  loading: boolean;
  error?: string;

  fetchLogs: (filter?: LogFilter) => Promise<void>;
  setFilter: (f: Partial<LogFilter>) => void;
  toggleAutoScroll: () => void;
  exportLogs: (filePath: string, filter?: LogFilter) => Promise<void>;
  clearLogs: () => Promise<void>;
}

export const useLogStore = create<LogState>((set, get) => ({
  entries: [],
  filter: {},
  autoScroll: true,
  loading: false,
  error: undefined,

  async fetchLogs(filter) {
    set({ loading: true, error: undefined });
    try {
      const params = new URLSearchParams();
      const f = filter ?? get().filter;
      if (f.serverId) params.set('serverId', f.serverId);
      if (f.level) params.set('level', f.level);
      if (f.keyword) params.set('keyword', f.keyword);
      const qs = params.toString();
      const res = await apiFetch<LogEntry[]>(`/api/logs${qs ? '?' + qs : ''}`);
      set({ entries: res.data ?? [], loading: false });
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
    }
  },

  setFilter(f) {
    set((s) => ({ filter: { ...s.filter, ...f } }));
  },

  toggleAutoScroll() {
    set((s) => ({ autoScroll: !s.autoScroll }));
  },

  async exportLogs(filePath, filter) {
    await apiFetch('/logs/export', {
      method: 'POST',
      body: JSON.stringify({ filePath, filter: filter ?? get().filter }),
    });
  },

  async clearLogs() {
    await apiFetch('/api/logs/clear', { method: 'POST' });
    set({ entries: [] });
  },
}));

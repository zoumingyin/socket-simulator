/**
 * useLogStore - 日志查看器 Zustand Store
 *
 * 数据获取策略：初始批量加载 + 后续单条实时推送（均通过 WebSocket）
 */

import { create } from 'zustand';
import type { LogEntry, LogFilter } from '../types/index';
import { apiFetch } from '../api/client';
import { adminSocket } from '../socket/AdminSocketManager';

const MAX_ENTRIES = 2000;

interface LogState {
  entries: LogEntry[];
  filter: LogFilter;
  autoScroll: boolean;
  loading: boolean;
  error?: string;

  fetchLogs: (filter?: LogFilter) => Promise<void>;
  appendLogEntry: (entry: LogEntry) => void;
  appendLogEntries: (entries: LogEntry[]) => void;
  setFilter: (f: Partial<LogFilter>) => void;
  toggleAutoScroll: () => void;
  exportLogs: (filePath: string, filter?: LogFilter) => Promise<void>;
  clearLogs: () => Promise<void>;
}

// 订阅日志事件（store 外部，只注册一次）
let logSubscribed = false;
function subscribeLogUpdates(): void {
  if (logSubscribed) return;
  logSubscribed = true;

  // 单条日志推送
  adminSocket.subscribe('log_update', (data) => {
    const entry = data as LogEntry;
    useLogStore.getState().appendLogEntry(entry);
  });

  // 批量日志推送（初始加载）
  adminSocket.subscribe('log_batch', (data) => {
    const entries = data as LogEntry[];
    useLogStore.getState().appendLogEntries(entries);
  });
}

export const useLogStore = create<LogState>((set, get) => ({
  entries: [],
  filter: {},
  autoScroll: true,
  loading: false,
  error: undefined,

  async fetchLogs(filter) {
    subscribeLogUpdates();
    // HTTP 作为首次加载兜底
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

  /** 追加单条日志（来自 Socket 推送） */
  appendLogEntry(entry: LogEntry) {
    set((s) => {
      const next = [...s.entries, entry];
      return { entries: next.length > MAX_ENTRIES ? next.slice(-MAX_ENTRIES) : next };
    });
  },

  /** 批量追加日志（来自 Socket 初始加载） */
  appendLogEntries(entries: LogEntry[]) {
    set((s) => {
      // 避免与 HTTP 初始加载重复
      if (s.entries.length >= entries.length) return {};
      const next = [...s.entries, ...entries];
      return { entries: next.length > MAX_ENTRIES ? next.slice(-MAX_ENTRIES) : next };
    });
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

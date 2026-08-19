/**
 * useLogStore - 日志查看器 Zustand Store
 *
 * 数据获取策略：初始批量加载 + 后续单条实时推送（均通过 WebSocket）
 */

import { create } from 'zustand';
import type { LogEntry, LogFilter, LogLevel } from '../types/index';
import { api } from '../api';
import { adminSocket } from '../socket/AdminSocketManager';

const MAX_ENTRIES = 2000;

/** 与后端 `log_manager::get_entries` 对齐的内存过滤（serverId / level 下限 / keyword） */
const LEVEL_ORDER: LogLevel[] = ['DEBUG', 'INFO', 'WARN', 'ERROR'];

function filterEntries(entries: LogEntry[], filter: LogFilter): LogEntry[] {
  let result = entries;
  if (filter.serverId) {
    const sid = filter.serverId;
    result = result.filter((e) => e.serverId === sid);
  }
  if (filter.level) {
    const minIdx = LEVEL_ORDER.indexOf(filter.level);
    result = result.filter((e) => LEVEL_ORDER.indexOf(e.level ?? 'DEBUG') >= minIdx);
  }
  if (filter.keyword) {
    const kw = filter.keyword.toLowerCase();
    result = result.filter(
      (e) =>
        (e.message ?? '').toLowerCase().includes(kw) ||
        (e.serverId?.toLowerCase().includes(kw) ?? false),
    );
  }
  return result;
}

interface LogState {
  entries: LogEntry[];
  /** 历史持久化日志（P1-4：SQLite 分页拉取，早于实时流） */
  historyEntries: LogEntry[];
  historyTotal: number;
  historyLoading: boolean;
  filter: LogFilter;
  autoScroll: boolean;
  loading: boolean;
  error?: string;

  fetchLogs: (filter?: LogFilter) => Promise<void>;
  /** 拉取历史持久化日志（按当前 filter，单页 500 条，去重合并进 historyEntries） */
  fetchHistory: () => Promise<void>;
  appendLogEntry: (entry: LogEntry) => void;
  appendLogEntries: (entries: LogEntry[]) => void;
  setFilter: (f: Partial<LogFilter>) => void;
  toggleAutoScroll: () => void;
  exportLogs: (filter?: LogFilter) => Promise<void>;
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
  historyEntries: [],
  historyTotal: 0,
  historyLoading: false,
  filter: {},
  autoScroll: true,
  loading: false,
  error: undefined,

  async fetchLogs(filter) {
    subscribeLogUpdates();
    // HTTP 作为首次加载兜底
    set({ loading: true, error: undefined });
    try {
      const f = filter ?? get().filter;
      const res = await api.logs.list(f);
      set({ entries: res.data ?? [], loading: false });
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
    }
  },

  async fetchHistory() {
    set({ historyLoading: true });
    try {
      const f = get().filter;
      const res = await api.logs.persisted({ ...f, limit: 500, offset: 0 });
      const items = res.data?.items ?? [];
      set((s) => {
        // 按 id 去重合并（历史早于实时流，排前面）
        const known = new Set(s.entries.map((e) => e.id).concat(s.historyEntries.map((e) => e.id)));
        const fresh = items.filter((e) => !known.has(e.id));
        return {
          historyEntries: [...fresh.reverse(), ...s.historyEntries],
          historyTotal: res.data?.total ?? 0,
          historyLoading: false,
        };
      });
    } catch (e) {
      set({ historyLoading: false, error: (e as Error).message });
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

  /** 导出日志：按当前 filter 过滤内存中的 entries，前端生成 JSON 文件下载 */
  async exportLogs(filter) {
    const f = filter ?? get().filter;
    const data = JSON.stringify(filterEntries(get().entries, f), null, 2);
    const blob = new Blob([data], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `socket-logs-${new Date().toISOString().split('T')[0]}.json`;
    a.click();
    URL.revokeObjectURL(url);
  },

  async clearLogs() {
    await api.logs.clear();
    set({ entries: [] });
  },
}));

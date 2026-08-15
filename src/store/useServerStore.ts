/**
 * useServerStore - 服务管理 Zustand Store
 *
 * 管理两类数据，更新通道不同，切勿混淆：
 * - 配置列表（servers: ServerConfig）：初始 HTTP 拉取（fetchServers），每次增删改后经
 *   HTTP 写回并本地 set 乐观更新；**不**经 WebSocket 推送（后端无 server_update 事件）。
 *   含 per-service Mock 配置（mockEnabled / mockRules / mockDefault*），同样走 HTTP 变更。
 * - 运行时状态（runtimes: ServerRuntime）：首屏 HTTP 兜底（fetchRuntimes），之后由 Admin WS
 *   的 runtime_update 事件持续推送覆盖，无需手动轮询。
 */
import { create } from 'zustand';
import type { ServerConfig, ServerRuntime } from '../types/index';
import { api } from '../api';
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
      const res = await api.servers.list();
      set({ list: res.data ?? [], loading: false });
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
    }
  },

  async fetchRuntimes() {
    subscribeRuntimeUpdates();
    // HTTP 作为首次加载兜底
    try {
      const res = await api.servers.runtimes();
      if (res.data) set({ runtimes: res.data });
    } catch {
      // 忽略 HTTP 失败，Socket 推送会更新数据
    }
  },

  async addServer(config) {
    const res = await api.servers.add(config);
    if (!res.data) throw new Error(res.error ?? '添加失败');
    set((s) => ({ list: [...s.list, res.data!] }));
    return res.data;
  },

  async updateServer(id, patch) {
    const current = get().list.find((srv) => srv.id === id);
    if (!current) throw new Error('服务不存在');
    const next: ServerConfig = { ...current, ...patch, id };
    const res = await api.servers.update(next);
    const saved = res.data ?? next;
    set((s) => ({
      list: s.list.map((srv) => (srv.id === id ? saved : srv)),
    }));
  },

  async removeServer(id) {
    await api.servers.remove(id);
    set((s) => ({
      list: s.list.filter((srv) => srv.id !== id),
      runtimes: (() => { const { [id]: _, ...rest } = s.runtimes; return rest; })(),
    }));
  },

  async startServer(id) {
    await api.servers.start(id);
    // Socket 推送会自动更新 runtimes
  },

  async stopServer(id) {
    await api.servers.stop(id);
  },

  async restartServer(id) {
    await api.servers.restart(id);
  },

  async startAll() {
    await api.servers.startAll();
  },

  async stopAll() {
    await api.servers.stopAll();
  },

  async restartAll() {
    await api.servers.restartAll();
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

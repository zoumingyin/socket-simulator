/**
 * useClientStore - 客户端管理 Zustand Store
 *
 * 数据获取策略：初始加载使用 HTTP，后续更新通过 WebSocket 实时推送
 */

import { create } from 'zustand';
import type { ClientInfo, SendMessageRequest } from '../types/index';
import { api } from '../api';
import { adminSocket } from '../socket/AdminSocketManager';

interface ClientState {
  list: ClientInfo[];
  loading: boolean;
  error?: string;

  fetchClients: (serverId?: string) => Promise<void>;
  sendMessage: (req: SendMessageRequest) => Promise<void>;
  broadcast: (req: SendMessageRequest) => Promise<void>;
  disconnectClient: (serverId: string, clientId: string) => Promise<void>;
  search: (keyword: string) => ClientInfo[];
}

// 订阅 client_update 事件
let clientSubscribed = false;
function subscribeClientUpdates(): void {
  if (clientSubscribed) return;
  clientSubscribed = true;

  adminSocket.subscribe('client_update', (data) => {
    const clients = data as ClientInfo[];
    useClientStore.setState({ list: clients });
  });
}

export const useClientStore = create<ClientState>((set, get) => ({
  list: [],
  loading: false,
  error: undefined,

  async fetchClients(serverId) {
    subscribeClientUpdates();
    // HTTP 首次加载
    set({ loading: true, error: undefined });
    try {
      const res = await api.clients.list(serverId);
      set({ list: res.data ?? [], loading: false });
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
    }
  },

  async sendMessage(req) {
    await api.clients.send(req);
  },

  async broadcast(req) {
    await api.clients.send({ ...req, targetType: 'broadcast' });
  },

  async disconnectClient(serverId, clientId) {
    await api.clients.disconnect(`${serverId}___${clientId}`);
    set((s) => ({ list: s.list.filter((c) => c.id !== clientId) }));
  },

  search(keyword) {
    const lower = keyword.toLowerCase();
    return get().list.filter(
      (c) =>
        c.id.toLowerCase().includes(lower) ||
        c.socketId.toLowerCase().includes(lower) ||
        c.ipAddress.toLowerCase().includes(lower)
    );
  },
}));

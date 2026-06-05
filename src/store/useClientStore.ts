/**
 * useClientStore - 客户端管理 Zustand Store
 */
import { create } from 'zustand';
import type { ClientInfo, SendMessageRequest, ClientGroup } from '../types/index';
import { apiFetch } from '../api/client';

interface ClientState {
  list: ClientInfo[];
  groups: ClientGroup[];
  loading: boolean;
  error?: string;

  fetchClients: (serverId?: string) => Promise<void>;
  fetchGroups: () => Promise<void>;
  sendMessage: (req: SendMessageRequest) => Promise<void>;
  broadcast: (req: SendMessageRequest) => Promise<void>;
  disconnectClient: (serverId: string, clientId: string) => Promise<void>;
  search: (keyword: string) => ClientInfo[];
}

export const useClientStore = create<ClientState>((set, get) => ({
  list: [],
  groups: [],
  loading: false,
  error: undefined,

  async fetchClients(serverId) {
    set({ loading: true, error: undefined });
    try {
      const url = serverId ? `/api/clients?serverId=${serverId}` : '/api/clients';
      const res = await apiFetch<ClientInfo[]>(url);
      set({ list: res.data ?? [], loading: false });
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
    }
  },

  async fetchGroups() {
    try {
      const res = await apiFetch<ClientGroup[]>('/api/client-groups');
      set({ groups: res.data ?? [] });
    } catch { /* ignore */ }
  },

  async sendMessage(req) {
    await apiFetch('/client/send', {
      method: 'POST',
      body: JSON.stringify(req),
    });
  },

  async broadcast(req) {
    await apiFetch('/client/send', {
      method: 'POST',
      body: JSON.stringify({ ...req, targetType: 'broadcast' }),
    });
  },

  async disconnectClient(serverId, clientId) {
    await apiFetch(`/client/disconnect`, {
      method: 'POST',
      body: JSON.stringify({ clientId: `${serverId}___${clientId}` }),
    });
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

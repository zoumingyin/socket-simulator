/**
 * useMessageStore - 消息中心 Zustand Store
 */
import { create } from 'zustand';
import type { MessageTemplate, SendMessageRequest, MessageType } from '../types/index';
import { apiFetch } from '../api/client';

interface MessageState {
  templates: MessageTemplate[];
  sending: boolean;
  error?: string;

  fetchTemplates: () => Promise<void>;
  addTemplate: (t: Omit<MessageTemplate, 'id' | 'createdAt' | 'updatedAt'>) => Promise<void>;
  updateTemplate: (id: string, patch: Partial<MessageTemplate>) => Promise<void>;
  removeTemplate: (id: string) => Promise<void>;
  sendMessage: (req: SendMessageRequest) => Promise<void>;
  broadcast: (req: Omit<SendMessageRequest, 'targetType'>) => Promise<void>;
  formatJSON: (content: string) => { ok: boolean; formatted?: string; error?: string };
  validateJSON: (content: string) => { valid: boolean; error?: string };
  minifyJSON: (content: string) => { ok: boolean; minified?: string; error?: string };
}

export const useMessageStore = create<MessageState>((set, get) => ({
  templates: [],
  sending: false,
  error: undefined,

  async fetchTemplates() {
    try {
      const res = await apiFetch<MessageTemplate[]>('/templates');
      set({ templates: res.data ?? [] });
    } catch (e) {
      set({ error: (e as Error).message });
    }
  },

  async addTemplate(t) {
    const res = await apiFetch<MessageTemplate>('/template/save', {
      method: 'POST',
      body: JSON.stringify(t),
    });
    if (res.data) set((s) => ({ templates: [...s.templates, res.data!] }));
  },

  async updateTemplate(id, patch) {
    const res = await apiFetch<MessageTemplate>('/template/save', {
      method: 'POST',
      body: JSON.stringify({ id, ...patch }),
    });
    if (res.data) {
      set((s) => ({ templates: s.templates.map((t) => (t.id === id ? res.data! : t)) }));
    }
  },

  async removeTemplate(id) {
    await apiFetch('/template/delete', {
      method: 'POST',
      body: JSON.stringify({ id }),
    });
    set((s) => ({ templates: s.templates.filter((t) => t.id !== id) }));
  },

  async sendMessage(req) {
    set({ sending: true, error: undefined });
    try {
      await apiFetch('/send-message', {
        method: 'POST',
        body: JSON.stringify(req),
      });
    } catch (e) {
      set({ error: (e as Error).message });
      throw e;
    } finally {
      set({ sending: false });
    }
  },

  async broadcast(req) {
    set({ sending: true, error: undefined });
    try {
      await apiFetch('/client/send', {
        method: 'POST',
        body: JSON.stringify({ ...req, targetType: 'broadcast' }),
      });
    } catch (e) {
      set({ error: (e as Error).message });
      throw e;
    } finally {
      set({ sending: false });
    }
  },

  formatJSON(content) {
    try {
      const parsed = JSON.parse(content);
      return { ok: true, formatted: JSON.stringify(parsed, null, 2) };
    } catch (e) {
      return { ok: false, error: (e as Error).message };
    }
  },

  validateJSON(content) {
    try {
      JSON.parse(content);
      return { valid: true };
    } catch (e) {
      return { valid: false, error: (e as Error).message };
    }
  },

  minifyJSON(content) {
    try {
      const parsed = JSON.parse(content);
      return { ok: true, minified: JSON.stringify(parsed) };
    } catch (e) {
      return { ok: false, error: (e as Error).message };
    }
  },
}));

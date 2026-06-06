/**
 * useMessageStore - 消息中心 Zustand Store
 */
import { create } from 'zustand';
import type { SendMessageRequest } from '../types/index';
import { apiFetch } from '../api/client';

interface MessageState {
  sending: boolean;
  error?: string;

  sendMessage: (req: SendMessageRequest) => Promise<void>;
  broadcast: (req: Omit<SendMessageRequest, 'targetType'>) => Promise<void>;
  formatJSON: (content: string) => { ok: boolean; formatted?: string; error?: string };
  validateJSON: (content: string) => { valid: boolean; error?: string };
  minifyJSON: (content: string) => { ok: boolean; minified?: string; error?: string };
}

export const useMessageStore = create<MessageState>((set) => ({
  sending: false,
  error: undefined,

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

  formatJSON(content: string) {
    try {
      const parsed = JSON.parse(content);
      return { ok: true, formatted: JSON.stringify(parsed, null, 2) };
    } catch (e) {
      return { ok: false, error: (e as Error).message };
    }
  },

  validateJSON(content: string) {
    try {
      JSON.parse(content);
      return { valid: true };
    } catch (e) {
      return { valid: false, error: (e as Error).message };
    }
  },

  minifyJSON(content: string) {
    try {
      const parsed = JSON.parse(content);
      return { ok: true, minified: JSON.stringify(parsed) };
    } catch (e) {
      return { ok: false, error: (e as Error).message };
    }
  },
}));

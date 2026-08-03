/**
 * useMessageStore - 消息中心 Zustand Store
 */
import { create } from 'zustand';
import type { SendMessageRequest, SavedMessage } from '../types/index';
import { apiFetch } from '../api/client';

const SAVED_MESSAGES_KEY = 'ssm:savedMessages';

/** 从 localStorage 读取已保存消息（容错：解析失败/非数组时回退为空数组） */
function loadSavedMessages(): SavedMessage[] {
  try {
    const raw = localStorage.getItem(SAVED_MESSAGES_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? (parsed as SavedMessage[]) : [];
  } catch {
    return [];
  }
}

/** 持久化已保存消息到 localStorage（容错：忽略配额/序列化错误） */
function persistSavedMessages(list: SavedMessage[]): void {
  try {
    localStorage.setItem(SAVED_MESSAGES_KEY, JSON.stringify(list));
  } catch {
    /* 忽略配额或序列化错误 */
  }
}

interface MessageState {
  sending: boolean;
  error?: string;
  savedMessages: SavedMessage[];

  sendMessage: (req: SendMessageRequest) => Promise<void>;
  broadcast: (req: Omit<SendMessageRequest, 'targetType'>) => Promise<void>;
  /** 保存一条消息到本地（localStorage），新消息置顶 */
  saveMessage: (msg: Omit<SavedMessage, 'id' | 'createdAt'>) => void;
  /** 删除一条已保存消息 */
  deleteSavedMessage: (id: string) => void;
  formatJSON: (content: string) => { ok: boolean; formatted?: string; error?: string };
  validateJSON: (content: string) => { valid: boolean; error?: string };
  minifyJSON: (content: string) => { ok: boolean; minified?: string; error?: string };
}

export const useMessageStore = create<MessageState>((set) => ({
  sending: false,
  error: undefined,
  savedMessages: loadSavedMessages(),

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

  saveMessage(msg) {
    const item: SavedMessage = {
      id: `sm_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`,
      createdAt: new Date().toISOString(),
      ...msg,
    };
    set((s) => {
      const next = [item, ...s.savedMessages];
      persistSavedMessages(next);
      return { savedMessages: next };
    });
  },

  deleteSavedMessage(id) {
    set((s) => {
      const next = s.savedMessages.filter((m) => m.id !== id);
      persistSavedMessages(next);
      return { savedMessages: next };
    });
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

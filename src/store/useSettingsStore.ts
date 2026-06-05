/**
 * useSettingsStore - 系统设置 Zustand Store
 */
import { create } from 'zustand';
import type { SystemSettings, WindowConfig } from '../types/index';
import { apiFetch } from '../api/client';

interface SettingsState {
  systemSettings: SystemSettings;
  windowConfig: WindowConfig;
  loading: boolean;
  error?: string;

  fetchSettings: () => Promise<void>;
  updateSystemSettings: (patch: Partial<SystemSettings>) => Promise<void>;
  updateWindowConfig: (patch: Partial<WindowConfig>) => Promise<void>;
  exportConfig: () => Promise<Record<string, unknown>>;
  importConfig: (config: Record<string, unknown>) => Promise<void>;
}

const defaultSystemSettings: SystemSettings = {
  id: 'system',
  heartbeat: { enabled: true, pingInterval: 30000, pongTimeout: 90000 },
  wss: { enabled: false, certPath: '', keyPath: '' },
  ipAccess: { whitelist: [], blacklist: [] },
  autoStart: false,
  startMinimized: false,
  logRetentionDays: 7,
  maxConnectionsPerServer: 1000,
  updatedAt: new Date().toISOString(),
};

const defaultWindowConfig: WindowConfig = {
  width: 1280,
  height: 800,
  maximized: false,
};

export const useSettingsStore = create<SettingsState>((set, get) => ({
  systemSettings: defaultSystemSettings,
  windowConfig: defaultWindowConfig,
  loading: false,
  error: undefined,

  async fetchSettings() {
    set({ loading: true, error: undefined });
    try {
      const res = await apiFetch<{ systemSettings: SystemSettings; windowConfig: WindowConfig }>('/api/settings');
      if (res.data) {
        set({
          systemSettings: res.data.systemSettings ?? defaultSystemSettings,
          windowConfig: res.data.windowConfig ?? defaultWindowConfig,
          loading: false,
        });
      }
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
    }
  },

  async updateSystemSettings(patch) {
    const next = { ...get().systemSettings, ...patch, updatedAt: new Date().toISOString() };
    set({ systemSettings: next });
    await apiFetch('/api/settings', {
      method: 'POST',
      body: JSON.stringify({ systemSettings: next }),
    });
  },

  async updateWindowConfig(patch) {
    const next = { ...get().windowConfig, ...patch };
    set({ windowConfig: next });
    await apiFetch('/api/settings', {
      method: 'POST',
      body: JSON.stringify({ windowConfig: next }),
    });
  },

  async exportConfig() {
    const res = await apiFetch<Record<string, unknown>>('/api/export');
    return res.data ?? {};
  },

  async importConfig(config) {
    await apiFetch('/api/import', {
      method: 'POST',
      body: JSON.stringify(config),
    });
    // 重新加载
    await get().fetchSettings();
  },
}));

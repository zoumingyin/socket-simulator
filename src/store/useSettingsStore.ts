/**
 * useSettingsStore - 系统设置 Zustand Store
 */
import { create } from 'zustand';
import type { SystemSettings, WindowConfig } from '../types/index';
import { api } from '../api';

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
      const res = await api.settings.get();
      if (!res.success) {
        const errMsg = res.error || '加载设置失败';
        set({ loading: false, error: errMsg });
        console.error('[SettingsStore] fetchSettings 失败:', errMsg);
        return;
      }
      set({
        systemSettings: res.data?.systemSettings ?? defaultSystemSettings,
        windowConfig: res.data?.windowConfig ?? defaultWindowConfig,
        loading: false,
      });
    } catch (e) {
      const errMsg = (e as Error).message;
      set({ error: errMsg, loading: false });
      console.error('[SettingsStore] fetchSettings 异常:', errMsg);
    }
  },

  async updateSystemSettings(patch) {
    const prev = get().systemSettings;
    const next = { ...prev, ...patch, updatedAt: new Date().toISOString() };
    // 乐观更新
    set({ systemSettings: next, loading: true, error: undefined });
    try {
      const res = await api.settings.save({ systemSettings: next });
      if (!res.success) {
        // 回滚
        set({ systemSettings: prev, loading: false, error: res.error || '保存失败' });
        throw new Error(res.error || '保存设置失败');
      }
      // 保存成功后重新拉取验证
      await get().fetchSettings();
    } catch (e) {
      if (!(e instanceof Error && e.message === '保存设置失败')) {
        // 网络错误等，回滚乐观更新
        set({ systemSettings: prev, loading: false, error: (e as Error).message });
      }
      throw e;
    }
    set({ loading: false });
  },

  async updateWindowConfig(patch) {
    const prev = get().windowConfig;
    const next = { ...prev, ...patch };
    set({ windowConfig: next, loading: true, error: undefined });
    try {
      const res = await api.settings.save({ windowConfig: next });
      if (!res.success) {
        set({ windowConfig: prev, loading: false, error: res.error || '保存失败' });
        throw new Error(res.error || '保存窗口配置失败');
      }
      await get().fetchSettings();
    } catch (e) {
      if (!(e instanceof Error && e.message === '保存窗口配置失败')) {
        set({ windowConfig: prev, loading: false, error: (e as Error).message });
      }
      throw e;
    }
    set({ loading: false });
  },

  async exportConfig() {
    const res = await api.config.export();
    return res.data ?? {};
  },

  async importConfig(config) {
    await api.config.import(config);
    // 重新加载
    await get().fetchSettings();
  },
}));

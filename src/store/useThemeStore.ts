/**
 * useThemeStore - 主题切换 Zustand Store
 * 管理 light/dark 主题模式
 */
import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export type ThemeMode = 'light' | 'dark';

interface ThemeState {
  themeMode: ThemeMode;
  toggleTheme: () => void;
  setTheme: (mode: ThemeMode) => void;
}

export const useThemeStore = create<ThemeState>()(
  persist(
    (set, get) => ({
      themeMode: 'light',
      toggleTheme: () => {
        const next = get().themeMode === 'light' ? 'dark' : 'light';
        set({ themeMode: next });
      },
      setTheme: (mode: ThemeMode) => set({ themeMode: mode }),
    }),
    {
      name: 'socket-service-theme',
      partialize: (state) => ({ themeMode: state.themeMode }),
    }
  )
);

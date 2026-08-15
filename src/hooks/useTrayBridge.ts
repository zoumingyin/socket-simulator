import { useEffect } from 'react';
import { message } from 'antd';
import { api } from '../api/index.js';

/**
 * useTrayBridge — 监听 Tauri 桌面托盘菜单事件（启动/停止/重启全部服务）。
 * 仅在 Tauri 环境下生效；浏览器开发模式动态导入 @tauri-apps/api 失败会静默忽略。
 * 抽离自 AppLayout，避免布局组件内联过多桌面专属副作用。
 */
export function useTrayBridge(): void {
  useEffect(() => {
    let cancelled = false;
    let unlistenFns: (() => void)[] = [];

    const setupTrayListeners = async () => {
      try {
        const { listen } = await import('@tauri-apps/api/event');

        if (cancelled) return;

        const unlistenStart = await listen('tray-start-all', async () => {
          message.info('正在启动全部服务...');
          try {
            const res = await api.servers.startAll();
            if (res.success) {
              message.success('全部服务启动成功');
            } else {
              message.error(`启动失败: ${res.error || '未知错误'}`);
            }
          } catch (err: unknown) {
            const error = err instanceof Error ? err : new Error(String(err));
            message.error(`启动失败: ${error.message}`);
          }
        });

        if (cancelled) {
          unlistenStart();
          return;
        }

        const unlistenStop = await listen('tray-stop-all', async () => {
          message.info('正在停止全部服务...');
          try {
            const res = await api.servers.stopAll();
            if (res.success) {
              message.success('全部服务停止成功');
            } else {
              message.error(`停止失败: ${res.error || '未知错误'}`);
            }
          } catch (err: unknown) {
            const error = err instanceof Error ? err : new Error(String(err));
            message.error(`停止失败: ${error.message}`);
          }
        });

        if (cancelled) {
          unlistenStop();
          return;
        }

        const unlistenRestart = await listen('tray-restart-all', async () => {
          message.info('正在重启全部服务...');
          try {
            const res = await api.servers.restartAll();
            if (res.success) {
              message.success('全部服务重启成功');
            } else {
              message.error(`重启失败: ${res.error || '未知错误'}`);
            }
          } catch (err: unknown) {
            const error = err instanceof Error ? err : new Error(String(err));
            message.error(`启动失败: ${error.message}`);
          }
        });

        unlistenFns = [unlistenStart, unlistenStop, unlistenRestart];
      } catch {
        // 非 Tauri 环境（浏览器开发），忽略
        return;
      }
    };

    setupTrayListeners();

    return () => {
      cancelled = true;
      unlistenFns.forEach((fn) => fn());
    };
  }, []);
}

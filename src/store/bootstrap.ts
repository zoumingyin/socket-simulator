/**
 * bootstrap.ts — 应用级核心数据预热
 *
 * 在 App 挂载时一次性拉取所有核心列表（servers / runtimes / clients / events / settings），
 * 避免各页面各自 mount 时出现的「空数据竞态」——典型如消息中心依赖 clients + events，
 * 若用户先于数据返回进入该页，下拉框会是空的。
 *
 * 设计要点：
 * - 幂等：用模块级 `bootstrapped` 标志防止 StrictMode 双调用 / 重复进入重复发请求。
 * - 只负责首屏冷数据；各页面仍保留自身 WebSocket 订阅（实时推送）与手动刷新，
 *   本模块不替代它们，fail 时页面级 fetch 仍可重试。
 * - 使用 Promise.allSettled，任一接口失败不影响其它列表加载。
 */
import { useServerStore } from "./useServerStore.js";
import { useClientStore } from "./useClientStore.js";
import { useEventStore } from "./useEventStore.js";
import { useSettingsStore } from "./useSettingsStore.js";

let bootstrapped = false;

/** 是否已预热完成（供需要等待数据的场景判断） */
export const isBootstrapped = (): boolean => bootstrapped;

/**
 * 预热核心列表。幂等、可 await。
 */
export async function bootstrapCore(): Promise<void> {
  if (bootstrapped) return;
  bootstrapped = true;

  const server = useServerStore.getState();
  const client = useClientStore.getState();
  const event = useEventStore.getState();
  const settings = useSettingsStore.getState();

  await Promise.allSettled([
    server.fetchServers(),
    server.fetchRuntimes(),
    client.fetchClients(),
    event.fetchEvents(),
    settings.fetchSettings(),
  ]);
}

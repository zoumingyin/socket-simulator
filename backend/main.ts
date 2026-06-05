/**
 * main.ts - 后端入口
 * 串联所有 Manager，初始化传输层，启动 REST API
 */
import { EventEmitter } from 'events';
import type { ServerConfig, LogEntry, ServerRuntime, ClientInfo, LogFilter } from './src/types/index';
import { ServiceManager } from './manager/ServiceManager.js';
import { ClientManager } from './manager/ClientManager.js';
import { EventManager } from './manager/EventManager.js';
import { LogManager } from './manager/LogManager.js';
import { ConfigManager } from './manager/ConfigManager.js';
import { startApiServer } from './api/index.js';

export class SocketServiceApp extends EventEmitter {
  public readonly serviceManager = new ServiceManager();
  public readonly clientManager = new ClientManager();
  public readonly eventManager = new EventManager();
  public readonly logManager = new LogManager();
  public readonly configManager = new ConfigManager();

  private initialized = false;

  async init(): Promise<void> {
    if (this.initialized) return;

    // 1. 加载配置
    this.configManager.init();
    const servers: ServerConfig[] = this.configManager.getServers();
    const events = this.configManager.getEvents();

    // 2. 加载事件配置到 EventManager
    this.eventManager.loadEvents(events);

    // 3. 加载服务配置到 ServiceManager
    const autoStartIds: string[] = [];
    for (const cfg of servers) {
      if (cfg.autoStart) autoStartIds.push(cfg.id);
      this.eventManager.registerServerConfig(cfg);
    }
    this.serviceManager.setClientManager(this.clientManager);
    this.serviceManager.loadConfig(servers, autoStartIds);

    // 4. 监听 ServiceManager 的传输层事件
    this.setupServiceListeners();

    // 5. 监听日志
    this.setupLogListeners();

    this.initialized = true;
  }

  private setupServiceListeners(): void {
    this.serviceManager.on('server_started', (serverId: string) => {
      const transport = this.serviceManager.getTransport(serverId);
      if (!transport) return;
      this.clientManager.registerTransport(serverId, transport);
      this.eventManager.registerTransport(serverId, transport);
      this.emitLog('INFO', 'server_started', `服务 ${serverId} 已启动`, serverId);
    });

    this.serviceManager.on('server_stopped', (serverId: string) => {
      this.clientManager.unregisterTransport(serverId);
      this.eventManager.unregisterTransport(serverId);
      this.emitLog('INFO', 'server_stopped', `服务 ${serverId} 已停止`, serverId);
    });
  }

  private setupLogListeners(): void {
    const toEntry = (data: unknown): LogEntry => {
      // 已经是 LogEntry（有 id + timestamp）直接返回
      if (data && typeof data === 'object' && 'id' in (data as object)) {
        return data as LogEntry;
      }
      // 是 { level, event, message, serverId? } 对象 → 补字段
      if (data && typeof data === 'object') {
        const d = data as Record<string, unknown>;
        return {
          id: crypto.randomUUID(),
          timestamp: new Date().toISOString(),
          level: (d.level as LogEntry['level']) ?? 'INFO',
          event: (d.event as string) ?? 'unknown',
          message: (d.message as string) ?? String(data),
          serverId: d.serverId as string | undefined,
        };
      }
      // 是字符串 → 包装成 LogEntry
      return {
        id: crypto.randomUUID(),
        timestamp: new Date().toISOString(),
        level: 'INFO',
        event: 'log',
        message: String(data),
        serverId: undefined,
      };
    };

    this.serviceManager.on('log', (data: unknown) => {
      this.logManager.addEntry(toEntry(data));
    });
    this.clientManager.on('log', (data: unknown) => {
      this.logManager.addEntry(toEntry(data));
    });
    this.eventManager.on('log', (data: unknown) => {
      this.logManager.addEntry(toEntry(data));
    });
  }

  private emitLog(level: 'INFO'|'WARN'|'ERROR'|'DEBUG', event: string, message: string, serverId?: string): void {
    const fullEntry: LogEntry = {
      id: crypto.randomUUID(),
      timestamp: new Date().toISOString(),
      level,
      event,
      message,
      serverId,
    };
    this.logManager.addEntry(fullEntry);
  }

  /** 供 REST API / MCP 调用 */
  getServerRuntimes(): Record<string, ServerRuntime> {
    const result: Record<string, ServerRuntime> = {};
    for (const rt of this.serviceManager.getAllRuntimes()) {
      result[rt.id] = rt;
    }
    return result;
  }

  getAllClients(): ClientInfo[] {
    return this.clientManager.getClients();
  }

  getLogs(filter?: LogFilter): LogEntry[] {
    return this.logManager.getEntries(filter);
  }

  async shutdown(): Promise<void> {
    await this.serviceManager.stopAll();
    this.emitLog('INFO', 'shutdown', '应用已关闭');
  }
}

/** 单例导出 */
let appInstance: SocketServiceApp | null = null;

export async function getApp(): Promise<SocketServiceApp> {
  if (!appInstance) {
    appInstance = new SocketServiceApp();
    await appInstance.init();
  }
  return appInstance;
}

/** 启动所有服务（供外部调用或直接使用） */
export async function startAll(): Promise<void> {
  await getApp();
  startApiServer();
  console.log('[main] SocketServiceApp 已启动，API 监听端口 3080');
}

// 全局未捕获异常处理——输出详细信息到 stderr
process.on('uncaughtException', (err, origin) => {
  console.error('[FATAL] uncaughtException', { origin, message: err.message, stack: err.stack });
});
process.on('unhandledRejection', (reason, promise) => {
  console.error('[FATAL] unhandledRejection', reason);
});

// ESM 检测是否直接运行此文件
const isMain =
  process.argv[1]?.endsWith('main.js') ||
  process.argv[1]?.endsWith('main.ts') ||
  process.argv[1]?.endsWith('start.js') ||
  process.argv[1]?.endsWith('start.ts');

if (isMain) {
  startAll().catch(err => {
    console.error('[main] 启动失败:', err);
    process.exit(1);
  });
}

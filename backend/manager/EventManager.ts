/**
 * EventManager - 事件管理
 * 支持动态新增/编辑/删除/启用/禁用事件
 * 同时支持 WebSocket 和 Socket.IO 事件格式
 * 支持事件轮询：启用后按间隔自动广播 defaultMessage
 */
import { EventEmitter } from 'events';
import { nanoid } from 'nanoid';
import type {
  EventConfig,
  ServerConfig,
  EventStatus,
} from '../src/types/index.js';
import type { ITransport } from '../src/types/index.js';
import { SocketIOTransport } from '../transport/socketio/SocketIOTransport.js';

/** 轮询定时器条目 */
interface PollingEntry {
  timer: ReturnType<typeof setInterval>;
  config: EventConfig;
}

export class EventManager extends EventEmitter {
  private events = new Map<string, EventConfig>();
  private serverConfigs = new Map<string, ServerConfig>();
  private transportMap = new Map<string, ITransport>();
  /** 轮询定时器 Map<eventId, PollingEntry> */
  private pollingTimers = new Map<string, PollingEntry>();

  /** 消息发送回调：当事件触发广播消息时，通知 ServiceManager 更新 sentMessages */
  private onMessageSent?: (serverId: string) => void;

  setOnMessageSent(callback: (serverId: string) => void): void {
    this.onMessageSent = callback;
  }

  // ==================== 轮询管理 ====================

  /** 解析消息内容：尝试 JSON.parse，失败则作为纯文本包装 */
  private parseMessage(msg: string): Record<string, unknown> {
    try {
      return JSON.parse(msg);
    } catch {
      return { message: msg };
    }
  }

  /** 启动单个事件的轮询定时器 */
  private startPolling(evt: EventConfig): void {
    if (!evt.pollingEnabled || !evt.pollingInterval || evt.pollingInterval <= 0) return;
    // 先清除已有定时器
    this.stopPolling(evt.id);

    const ms = evt.pollingInterval * 1000;
    const timer = setInterval(() => {
      const transport = this.transportMap.get(evt.serverId);
      if (!transport) return;
      try {
        const data = evt.defaultMessage ? this.parseMessage(evt.defaultMessage) : {};
        this.onMessageSent?.(evt.serverId);
        transport.broadcast(evt.name, data).catch((err: Error) => {
          console.error(`[EventManager] 轮询广播失败 event=${evt.name}:`, err.message);
        });
      } catch (e) {
        console.error(`[EventManager] 轮询消息解析失败 event=${evt.name}:`, (e as Error).message);
      }
    }, ms);

    this.pollingTimers.set(evt.id, { timer, config: evt });
  }

  /** 停止单个事件的轮询定时器 */
  private stopPolling(eventId: string): void {
    const entry = this.pollingTimers.get(eventId);
    if (entry) {
      clearInterval(entry.timer);
      this.pollingTimers.delete(eventId);
    }
  }

  /** 刷新所有轮询（事件配置变更时调用） */
  refreshAllPolling(): void {
    // 停止所有现有轮询
    for (const id of this.pollingTimers.keys()) {
      this.stopPolling(id);
    }
    // 重新启动所有符合条件的轮询
    for (const evt of this.events.values()) {
      if (evt.status === 'enabled' && evt.pollingEnabled) {
        if (this.transportMap.has(evt.serverId)) {
          this.startPolling(evt);
        }
      }
    }
  }

  /** 服务启动/停止时同步轮询状态（由 ServiceManager 调用） */
  onServerStatusChange(serverId: string, running: boolean): void {
    if (running) {
      for (const evt of this.events.values()) {
        if (evt.serverId === serverId && evt.status === 'enabled' && evt.pollingEnabled) {
          this.startPolling(evt);
        }
      }
    } else {
      for (const [id, entry] of this.pollingTimers.entries()) {
        if (entry.config.serverId === serverId) {
          this.stopPolling(id);
        }
      }
    }
  }

  // ==================== 配置加载 ====================

  /** 加载事件配置（应用启动时调用） */
  loadEvents(events: EventConfig[]): void {
    this.events.clear();
    for (const evt of events) {
      this.events.set(evt.id, evt);
    }
    // 加载完成后刷新轮询
    setTimeout(() => this.refreshAllPolling(), 0);
  }

  /** 注册服务端配置（用于判断协议类型） */
  registerServerConfig(config: ServerConfig): void {
    this.serverConfigs.set(config.id, config);
  }

  /** 注册传输层（ServiceManager 启动时调用） */
  registerTransport(serverId: string, transport: ITransport): void {
    this.transportMap.set(serverId, transport);
    this.setupSocketIOEvents(serverId, transport);
    // 传输层就绪后，启动该服务下所有启用轮询的事件
    this.onServerStatusChange(serverId, true);
  }

  unregisterTransport(serverId: string): void {
    this.onServerStatusChange(serverId, false);
    this.transportMap.delete(serverId);
  }

  // ==================== CRUD ====================

  getEvents(serverId?: string): EventConfig[] {
    const all = Array.from(this.events.values());
    return serverId ? all.filter((e) => e.serverId === serverId) : all;
  }

  getEvent(id: string): EventConfig | undefined {
    return this.events.get(id);
  }

  addEvent(
    config: Omit<EventConfig, 'id' | 'createdAt' | 'updatedAt'>
  ): EventConfig {
    const id = nanoid(12);
    const now = new Date().toISOString();
    const evt: EventConfig = { ...config, id, createdAt: now, updatedAt: now };
    this.events.set(id, evt);
    // 如果启用了轮询且服务正在运行，立即启动定时器
    if (evt.status === 'enabled' && evt.pollingEnabled && this.transportMap.has(evt.serverId)) {
      this.startPolling(evt);
    }
    return evt;
  }

  updateEvent(id: string, patch: Partial<EventConfig>): EventConfig | undefined {
    const existing = this.events.get(id);
    if (!existing) return undefined;
    const updated: EventConfig = { ...existing, ...patch, updatedAt: new Date().toISOString() };
    this.events.set(id, updated);
    // 轮询状态可能发生变化，重新评估
    if (updated.status === 'enabled' && updated.pollingEnabled && this.transportMap.has(updated.serverId)) {
      this.startPolling(updated);
    } else {
      this.stopPolling(id);
    }
    return updated;
  }

  removeEvent(id: string): boolean {
    this.stopPolling(id);
    return this.events.delete(id);
  }

  toggleEvent(id: string, status: EventStatus): EventConfig | undefined {
    return this.updateEvent(id, { status });
  }

  // ==================== 事件发送 ====================

  /**
   * 发送事件（兼容 WebSocket 和 Socket.IO 两种格式）
   * WebSocket: { "event": "chat", "data": { ... } }
   * Socket.IO: socket.emit("chat", data)
   */
  async emitEvent(
    serverId: string,
    targetClientId: string | null,
    eventName: string,
    data: Record<string, unknown>
  ): Promise<void> {
    const transport = this.transportMap.get(serverId);
    if (!transport) throw new Error(`Transport not found for server ${serverId}`);

    if (targetClientId) {
      await transport.send(targetClientId, eventName, data);
    } else {
      await transport.broadcast(eventName, data);
    }
    this.onMessageSent?.(serverId);
  }

  // ==================== Socket.IO 动态事件注册 ====================

  private setupSocketIOEvents(serverId: string, _transport: ITransport): void {
    // Socket.IO 事件通过 SocketIOTransport 的 onAny 统一处理，无需预注册
    // 事件启用/禁用时通过 event_config_changed 通知 ServiceManager 刷新
    const enabledEvents = this.getEvents(serverId).filter((e) => e.status === 'enabled');
    if (enabledEvents.length > 0) {
      this.emit('event_config_changed', serverId);
    }
  }

  /** 当事件被启用/禁用时，刷新 Socket.IO 监听 */
  refreshSocketIOListeners(serverId: string): void {
    const transport = this.transportMap.get(serverId);
    if (transport?.protocol === 'socket.io') {
      this.emit('event_config_changed', serverId);
    }
  }

  // ==================== 默认事件 ====================

  /** 初始化默认事件（connect / disconnect / message） */
  initDefaultEvents(serverId: string): EventConfig[] {
    const defaults: Array<Omit<EventConfig, 'id' | 'createdAt' | 'updatedAt'>> = [
      { serverId, name: 'connect', isDefault: true, status: 'enabled', description: '客户端连接事件' },
      { serverId, name: 'disconnect', isDefault: true, status: 'enabled', description: '客户端断开事件' },
      { serverId, name: 'message', isDefault: true, status: 'enabled', description: '收到消息事件' },
    ];

    const created: EventConfig[] = [];
    for (const d of defaults) {
      const exists = this.getEvents(serverId).some((e) => e.name === d.name && e.isDefault);
      if (!exists) {
        created.push(this.addEvent(d));
      }
    }
    return created;
  }
}

/**
 * AdminSocketManager — 统一的 WebSocket 连接管理模块
 *
 * 功能：
 * - 单例模式，全局唯一 Socket.IO 管理通道连接
 * - 自动重连（指数退避 + 随机抖动）
 * - 被动心跳检测（后端主动推送 heartbeat，前端 30s 超时判定）
 * - 发布/订阅模式的事件分发
 * - 连接状态追踪
 *
 * 后端对应事件：
 * - heartbeat       → 后端主动心跳（每 10s）
 * - runtime_update  → 运行时状态更新
 * - log_update      → 单条日志推送
 * - log_batch       → 批量日志推送（初始加载）
 * - client_update   → 客户端列表更新
 */
import { io, type Socket } from 'socket.io-client';

// ==================== 类型定义 ====================

export type SocketEvent =
  | 'heartbeat'
  | 'runtime_update'
  | 'log_update'
  | 'log_batch'
  | 'client_update'
  | 'connect'
  | 'disconnect'
  | 'connect_error';

export type ConnectionState = 'disconnected' | 'connecting' | 'connected' | 'reconnecting';

type EventHandler = (data: unknown) => void;
type StateListener = (state: ConnectionState) => void;

// ==================== 配置常量 ====================

const ADMIN_URL = 'http://localhost:3080';
const ADMIN_PATH = '/admin/socket.io';

/** 指数退避重连参数（Socket.IO 内置） */
const RECONNECT_DELAY_MIN = 1000;    // 初始 1 秒
const RECONNECT_DELAY_MAX = 30000;   // 最大 30 秒
const RECONNECT_JITTER = 0.3;       // 30% 随机抖动

/** 心跳超时：30 秒未收到后端 heartbeat → 判定连接断开 */
const HEARTBEAT_TIMEOUT = 30000;

// ==================== 单例实现 ====================

class AdminSocketManager {
  private socket: Socket | null = null;
  private eventHandlers = new Map<SocketEvent, Set<EventHandler>>();
  private stateListeners = new Set<StateListener>();
  private _connectionState: ConnectionState = 'disconnected';
  private _connecting = false;

  private heartbeatTimeoutTimer: ReturnType<typeof setTimeout> | null = null;
  private reconnectAttempt = 0;

  /** 获取当前连接状态 */
  get connectionState(): ConnectionState {
    return this._connectionState;
  }

  /** 是否已连接 */
  get isConnected(): boolean {
    return this._connectionState === 'connected';
  }

  // ==================== 连接管理 ====================

  /**
   * 建立连接（全局唯一，严格幂等）
   */
  connect(): void {
    if (this.socket?.connected) {
      return;
    }
    if (this._connecting) {
      console.log('[AdminSocket] 连接正在进行中，跳过重复请求');
      return;
    }

    this.cleanupSocket();

    this._connecting = true;
    this.setState('connecting');
    this.reconnectAttempt = 0;
    console.log('[AdminSocket] 正在建立全局管理通道连接...');

    this.socket = io(ADMIN_URL, {
      path: ADMIN_PATH,
      transports: ['websocket', 'polling'],
      reconnection: true,
      reconnectionAttempts: Infinity,
      reconnectionDelay: RECONNECT_DELAY_MIN,
      reconnectionDelayMax: RECONNECT_DELAY_MAX,
      randomizationFactor: RECONNECT_JITTER,
      timeout: 10000,
    });

    this.setupSocketListeners();
  }

  /** 断开连接（仅主动断开时调用，如组件卸载） */
  disconnect(): void {
    this._connecting = false;
    this.stopHeartbeatTimeout();
    this.cleanupSocket();
    this.setState('disconnected');
    console.log('[AdminSocket] 已主动断开全局管理通道');
  }

  /** 彻底销毁（清除所有监听器和状态，用于应用退出） */
  destroy(): void {
    this.disconnect();
    this.eventHandlers.clear();
    this.stateListeners.clear();
    console.log('[AdminSocket] 已彻底销毁 AdminSocketManager');
  }

  // ==================== 事件订阅 ====================

  subscribe(event: SocketEvent, handler: EventHandler): () => void {
    if (!this.eventHandlers.has(event)) {
      this.eventHandlers.set(event, new Set());
    }
    this.eventHandlers.get(event)!.add(handler);

    return () => {
      this.eventHandlers.get(event)?.delete(handler);
    };
  }

  onStateChange(listener: StateListener): () => void {
    this.stateListeners.add(listener);
    return () => {
      this.stateListeners.delete(listener);
    };
  }

  // ==================== 内部方法 ====================

  private setState(state: ConnectionState): void {
    if (this._connectionState === state) return;
    this._connectionState = state;
    this.stateListeners.forEach((fn) => fn(state));
  }

  private cleanupSocket(): void {
    if (this.socket) {
      this.socket.removeAllListeners();
      if (this.socket.connected) {
        this.socket.disconnect();
      }
      this.socket = null;
    }
  }

  private setupSocketListeners(): void {
    if (!this.socket) return;

    this.socket.on('connect', () => {
      console.log('[AdminSocket] 全局管理通道已连接 (单例)');
      this._connecting = false;
      this.setState('connected');
      this.reconnectAttempt = 0;
      // 连接成功后启动心跳超时检测
      this.resetHeartbeatTimeout();
      this.emitToHandlers('connect', undefined);
    });

    this.socket.on('disconnect', (reason: string) => {
      console.log('[AdminSocket] 管理通道断开:', reason);
      this.setState('disconnected');
      this.stopHeartbeatTimeout();
      this.emitToHandlers('disconnect', reason);

      if (reason === 'io server disconnect') {
        this.socket?.close();
      }
    });

    this.socket.on('connect_error', (err: Error) => {
      this._connecting = false;
      this.reconnectAttempt++;
      console.warn(
        `[AdminSocket] 连接失败 (第 ${this.reconnectAttempt} 次):`,
        err.message
      );
      this.setState('reconnecting');
      this.emitToHandlers('connect_error', err.message);
    });

    // ===== 后端主动推送的心跳 =====
    // 后端每 10s 推送 heartbeat → 前端收到后重置 30s 超时
    this.socket.on('heartbeat', () => {
      this.resetHeartbeatTimeout();
      // 回复 ack 让后端更新 lastHeartbeat 时间
      this.socket?.emit('heartbeat_ack');
    });

    // ===== 业务事件转发 =====
    const businessEvents: SocketEvent[] = [
      'runtime_update',
      'log_update',
      'log_batch',
      'client_update',
      // heartbeat 已在上方单独处理（需要调用 resetHeartbeatTimeout + reply ack）
    ];

    for (const evt of businessEvents) {
      this.socket.on(evt, (data: unknown) => {
        this.emitToHandlers(evt, data);
      });
    }
  }

  private emitToHandlers(event: SocketEvent, data: unknown): void {
    const handlers = this.eventHandlers.get(event);
    if (!handlers || handlers.size === 0) return;

    for (const handler of handlers) {
      try {
        handler(data);
      } catch (err) {
        console.error(`[AdminSocket] 事件处理器错误 (event=${event}):`, err);
      }
    }
  }

  // ==================== 心跳超时检测 ====================

  /**
   * 启动 / 重置心跳超时定时器
   * 每次收到后端 heartbeat 时调用，将超时倒计时重置为 30 秒
   * 若 30 秒内未收到 heartbeat → 判定断开，触发重连
   */
  private resetHeartbeatTimeout(): void {
    this.stopHeartbeatTimeout();

    this.heartbeatTimeoutTimer = setTimeout(() => {
      console.warn('[AdminSocket] 心跳超时（30s 未收到后端 heartbeat），判定连接断开');
      // 通过 disconnect() 清理，再通过 connect() 重建（利用指数退避）
      this.disconnect();
      this.connect();
    }, HEARTBEAT_TIMEOUT);
  }

  private stopHeartbeatTimeout(): void {
    if (this.heartbeatTimeoutTimer !== null) {
      clearTimeout(this.heartbeatTimeoutTimer);
      this.heartbeatTimeoutTimer = null;
    }
  }
}

// ==================== 导出单例 ====================

export const adminSocket = new AdminSocketManager();

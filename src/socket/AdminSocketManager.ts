/**
 * AdminSocketManager — 统一的 WebSocket 连接管理模块
 *
 * 功能：
 * - 单例模式，全局唯一 Socket.IO 管理通道连接
 * - 自动重连（指数退避 + 随机抖动）
 * - 应用层心跳保活（收到 pong 后清除超时定时器）
 * - 发布/订阅模式的事件分发
 * - 连接状态追踪
 *
 * 后端对应事件：
 * - runtime_update  → 运行时状态更新
 * - log_update      → 单条日志推送
 * - log_batch       → 批量日志推送（初始加载）
 * - client_update   → 客户端列表更新
 * - admin_pong      → 心跳响应
 */
import { io, type Socket } from 'socket.io-client';

// ==================== 类型定义 ====================

export type SocketEvent =
  | 'runtime_update'
  | 'log_update'
  | 'log_batch'
  | 'client_update'
  | 'admin_pong'
  | 'connect'
  | 'disconnect'
  | 'connect_error';

export type ConnectionState = 'disconnected' | 'connecting' | 'connected' | 'reconnecting';

type EventHandler = (data: unknown) => void;
type StateListener = (state: ConnectionState) => void;

// ==================== 配置常量 ====================

const ADMIN_URL = 'http://localhost:3080';
const ADMIN_PATH = '/admin/socket.io';

/** 指数退避重连参数（Socket.IO 内置，此处仅作说明） */
const RECONNECT_DELAY_MIN = 1000;   // 初始 1 秒
const RECONNECT_DELAY_MAX = 30000;   // 最大 30 秒
const RECONNECT_JITTER = 0.3;      // 30% 随机抖动

/** 心跳参数 */
const HEARTBEAT_INTERVAL = 30000;   // 30 秒发一次 ping（略小于 Socket.IO 默认 ping 35s）
const HEARTBEAT_TIMEOUT = 15000;    // 15 秒内未收到 pong 视为断线

// ==================== 单例实现 ====================

class AdminSocketManager {
  private socket: Socket | null = null;
  private eventHandlers = new Map<SocketEvent, Set<EventHandler>>();
  private stateListeners = new Set<StateListener>();
  private _connectionState: ConnectionState = 'disconnected';
  private _connecting = false; // 防止并发创建多个连接

  private heartbeatTimer: ReturnType<typeof setInterval> | null = null;
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
   * — 已连接 → 直接返回
   * — 正在连接中 → 直接返回（防竞态）
   * — 未连接 → 创建新连接
   */
  connect(): void {
    // 已连接或正在连接中，不重复创建
    if (this.socket?.connected) {
      return;
    }
    if (this._connecting) {
      console.log('[AdminSocket] 连接正在进行中，跳过重复请求');
      return;
    }

    // 清理旧连接（仅在已完全连接时才断开，避免 React Strict Mode 下中断握手报错）
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
    this.startHeartbeat();
  }

  /** 断开连接（仅主动断开时调用，如组件卸载） */
  disconnect(): void {
    this._connecting = false;
    this.stopHeartbeat();
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

  /** 订阅事件（多次调用同一事件+handler 不会重复注册） */
  subscribe(event: SocketEvent, handler: EventHandler): () => void {
    if (!this.eventHandlers.has(event)) {
      this.eventHandlers.set(event, new Set());
    }
    this.eventHandlers.get(event)!.add(handler);

    // 返回取消订阅函数
    return () => {
      this.eventHandlers.get(event)?.delete(handler);
    };
  }

  /** 订阅连接状态变化 */
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

  /** 清理 socket 实例（移除监听器 + 断开 + 清引用） */
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
      this.resetHeartbeatTimeout();
      this.emitToHandlers('connect', undefined);
    });

    this.socket.on('disconnect', (reason: string) => {
      console.log('[AdminSocket] 管理通道断开:', reason);
      this.setState('disconnected');
      this.stopHeartbeat();
      this.emitToHandlers('disconnect', reason);

      // 如果是服务器端主动断开，不自动重连（需用户手动重连）
      if (reason === 'io server disconnect') {
        // 停止 Socket.IO 内置重连
        this.socket?.close();
      }
      // 其他原因（transport close, ping timeout 等）由 Socket.IO 内置重连处理
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

    // 心跳响应：收到 pong 后清除超时定时器（关键！）
    this.socket.on('admin_pong', () => {
      this.resetHeartbeatTimeout();
    });

    // 注册业务事件转发
    const businessEvents: SocketEvent[] = [
      'runtime_update',
      'log_update',
      'log_batch',
      'client_update',
      // admin_pong 已在上方单独处理（需要调用 resetHeartbeatTimeout）
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

  // ==================== 心跳保活 ====================

  private startHeartbeat(): void {
    this.stopHeartbeat();

    this.heartbeatTimer = setInterval(() => {
      if (this.socket?.connected) {
        this.socket.emit('admin_ping');
        // 设置 pong 超时：15 秒内未收到 pong → 认为连接已死，触发重连
        this.heartbeatTimeoutTimer = setTimeout(() => {
          console.warn('[AdminSocket] 心跳超时（15s 未收到 pong），强制断开并重连');
          // 通过 disconnect() 清理，再通过 connect() 重建（利用指数退避）
          this.disconnect();
          this.connect();
        }, HEARTBEAT_TIMEOUT);
      }
    }, HEARTBEAT_INTERVAL);
  }

  private stopHeartbeat(): void {
    if (this.heartbeatTimer !== null) {
      clearInterval(this.heartbeatTimer);
      this.heartbeatTimer = null;
    }
    this.resetHeartbeatTimeout();
  }

  private resetHeartbeatTimeout(): void {
    if (this.heartbeatTimeoutTimer !== null) {
      clearTimeout(this.heartbeatTimeoutTimer);
      this.heartbeatTimeoutTimer = null;
    }
  }
}

// ==================== 导出单例 ====================

export const adminSocket = new AdminSocketManager();

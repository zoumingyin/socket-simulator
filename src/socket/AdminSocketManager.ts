/**
 * AdminSocketManager —— 统一的 WebSocket 连接管理模块（原生 WebSocket 版）
 *
 * 功能：
 * - 单例模式，全局唯一管理通道连接（ws://127.0.0.1:3080/admin/ws）
 * - 自动重连（指数退避 + 随机抖动）
 * - 被动心跳检测（后端主动推送 heartbeat，前端回复 heartbeat_ack，30s 超时判定）
 * - 发布/订阅模式的事件分发
 * - 连接状态追踪
 *
 * 后端对应事件（Rust /admin/ws）：
 * - heartbeat       → 后端主动心跳（每 10s）
 * - heartbeat_ack   → 前端回复（收到 heartbeat 后）
 * - runtime_update  → 运行时状态更新
 * - log_update      → 单条日志推送
 * - log_batch       → 批量日志推送（初始加载）
 * - client_update   → 客户端列表更新
 *
 * 注：本版已移除 socket.io-client 依赖，直接基于浏览器原生 WebSocket 实现。
 */
// 不再依赖 socket.io-client

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

/** 管理通道基础地址（http/https 自动转为 ws/wss） */
const API_BASE = import.meta.env.VITE_API_BASE || 'http://localhost:3080';
/** 管理 WebSocket 路径 */
const ADMIN_WS_PATH = import.meta.env.VITE_ADMIN_WS_PATH || '/admin/ws';

/** 构造连接地址：http(s)://host:port/path → ws(s)://host:port/path */
function buildWsUrl(): string {
  const base = API_BASE.replace(/^http/, 'ws');
  const sep = base.endsWith('/') || ADMIN_WS_PATH.startsWith('/') ? '' : '/';
  return `${base}${sep}${ADMIN_WS_PATH}`;
}

/** 指数退避重连参数 */
const RECONNECT_DELAY_MIN = 1000; // 初始 1 秒
const RECONNECT_DELAY_MAX = 30000; // 最大 30 秒
const RECONNECT_JITTER = 0.3; // 30% 随机抖动

/** 心跳超时：30 秒未收到后端 heartbeat → 判定连接断开并触发重连 */
const HEARTBEAT_TIMEOUT = 30000;

// ==================== 单例实现 ====================

class AdminSocketManager {
  private socket: WebSocket | null = null;
  private eventHandlers = new Map<SocketEvent, Set<EventHandler>>();
  private stateListeners = new Set<StateListener>();
  private _connectionState: ConnectionState = 'disconnected';

  private heartbeatTimeoutTimer: ReturnType<typeof setTimeout> | null = null;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private reconnectAttempt = 0;
  private manuallyClosed = false;

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
    if (this.socket && (this.socket.readyState === WebSocket.OPEN || this.socket.readyState === WebSocket.CONNECTING)) {
      return;
    }
    this.manuallyClosed = false;
    this.cleanupSocket();
    this.reconnectAttempt = 0;
    this.open();
  }

  /** 真正建立 WebSocket 连接 */
  private open(): void {
    const url = buildWsUrl();
    this.setState('connecting');
    console.log('[AdminSocket] 正在建立全局管理通道连接...', url);

    let ws: WebSocket;
    try {
      ws = new WebSocket(url);
    } catch (err) {
      console.error('[AdminSocket] 创建 WebSocket 失败:', err);
      this.scheduleReconnect();
      return;
    }
    this.socket = ws;

    ws.onopen = () => {
      console.log('[AdminSocket] 全局管理通道已连接 (单例)');
      this.reconnectAttempt = 0;
      this.setState('connected');
      this.resetHeartbeatTimeout();
      this.emitToHandlers('connect', undefined);
    };

    ws.onmessage = (ev: MessageEvent) => {
      let parsed: { event?: string; data?: unknown };
      try {
        parsed = JSON.parse(typeof ev.data === 'string' ? ev.data : String(ev.data));
      } catch {
        return;
      }
      const event = parsed.event;
      const data = parsed.data;
      if (!event) return;

      // 后端主动心跳：回复 ack + 重置超时
      if (event === 'heartbeat') {
        this.replyHeartbeatAck();
        this.resetHeartbeatTimeout();
        this.emitToHandlers('heartbeat', data);
        return;
      }

      if (
        event === 'runtime_update' ||
        event === 'log_update' ||
        event === 'log_batch' ||
        event === 'client_update'
      ) {
        this.emitToHandlers(event as SocketEvent, data);
      }
    };

    ws.onclose = (ev: CloseEvent) => {
      console.log('[AdminSocket] 管理通道断开:', ev.reason || `code=${ev.code}`);
      this.setState('disconnected');
      this.stopHeartbeatTimeout();
      this.emitToHandlers('disconnect', ev.reason || `code=${ev.code}`);
      if (!this.manuallyClosed) {
        this.scheduleReconnect();
      }
    };

    ws.onerror = (err: Event) => {
      this.reconnectAttempt++;
      console.warn(`[AdminSocket] 连接错误 (第 ${this.reconnectAttempt} 次):`, err);
      this.setState('reconnecting');
      this.emitToHandlers('connect_error', (err as ErrorEvent)?.message || 'websocket error');
    };
  }

  /** 断开连接（仅主动断开时调用，如组件卸载） */
  disconnect(): void {
    this.manuallyClosed = true;
    this.stopReconnect();
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
      this.socket.onopen = null;
      this.socket.onmessage = null;
      this.socket.onclose = null;
      this.socket.onerror = null;
      if (
        this.socket.readyState === WebSocket.OPEN ||
        this.socket.readyState === WebSocket.CONNECTING
      ) {
        this.socket.close();
      }
      this.socket = null;
    }
  }

  /** 回复心跳 ack */
  private replyHeartbeatAck(): void {
    this.sendRaw('heartbeat_ack');
  }

  /** 发送原始事件帧 */
  private sendRaw(event: string, data?: unknown): void {
    if (this.socket && this.socket.readyState === WebSocket.OPEN) {
      this.socket.send(JSON.stringify({ event, data: data ?? null }));
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

  // ==================== 重连调度 ====================

  private scheduleReconnect(): void {
    if (this.reconnectTimer !== null || this.manuallyClosed) return;
    this.setState('reconnecting');

    const base = Math.min(
      RECONNECT_DELAY_MAX,
      RECONNECT_DELAY_MIN * Math.pow(2, this.reconnectAttempt),
    );
    const jitter = base * RECONNECT_JITTER;
    const delay = Math.round(base + (Math.random() * 2 - 1) * jitter);

    console.log(`[AdminSocket] ${delay}ms 后重连（第 ${this.reconnectAttempt + 1} 次）`);
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.reconnectAttempt++;
      this.open();
    }, delay);
  }

  private stopReconnect(): void {
    if (this.reconnectTimer !== null) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
  }

  // ==================== 心跳超时检测 ====================

  /**
   * 启动 / 重置心跳超时定时器
   * 每次收到后端 heartbeat 时调用，将超时倒计时重置为 30 秒；
   * 若 30 秒内未收到 heartbeat → 判定断开，触发重连。
   */
  private resetHeartbeatTimeout(): void {
    this.stopHeartbeatTimeout();
    this.heartbeatTimeoutTimer = setTimeout(() => {
      console.warn('[AdminSocket] 心跳超时（30s 未收到后端 heartbeat），判定连接断开');
      this.cleanupSocket();
      this.scheduleReconnect();
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

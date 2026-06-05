/**
 * ServiceManager - 服务生命周期管理
 * 管理所有传输层实例的启动/停止/重启
 */
import { EventEmitter } from 'events';
import { nanoid } from 'nanoid';
import type {
  ServerConfig,
  ServerRuntime,
  ServerStatus,
  ProtocolType,
} from '../src/types/index';
import { WebSocketTransport } from '../transport/websocket/WebSocketTransport.js';
import { SocketIOTransport } from '../transport/socketio/SocketIOTransport.js';
import type { ITransport } from '../src/types/index';
import type { ClientManager } from './ClientManager';

export class ServiceManager extends EventEmitter {
  private servers = new Map<string, ServerConfig>();
  private runtimes = new Map<string, ServerRuntime>();
  private transports = new Map<string, ITransport>();
  private clientManager!: ClientManager;

  /** 注入 ClientManager，用于连接时注册客户端 */
  setClientManager(cm: ClientManager): void {
    this.clientManager = cm;
  }

  /** 加载配置并可选自动启动 */
  loadConfig(servers: ServerConfig[], autoStartIds: string[]): void {
    for (const cfg of servers) {
      this.servers.set(cfg.id, cfg);
      this.runtimes.set(cfg.id, this.createInitialRuntime(cfg.id));
    }
    for (const id of autoStartIds) {
      this.startServer(id).catch(() => {});
    }
  }

  getServers(): ServerConfig[] {
    return Array.from(this.servers.values());
  }

  getServer(id: string): ServerConfig | undefined {
    return this.servers.get(id);
  }

  getRuntime(id: string): ServerRuntime | undefined {
    return this.runtimes.get(id);
  }

  getAllRuntimes(): ServerRuntime[] {
    return Array.from(this.runtimes.values());
  }

  addServer(config: Omit<ServerConfig, 'id' | 'createdAt' | 'updatedAt'>): ServerConfig {
    const id = nanoid(12);
    const now = new Date().toISOString();
    const server: ServerConfig = {
      ...config,
      id,
      createdAt: now,
      updatedAt: now,
    };
    this.servers.set(id, server);
    this.runtimes.set(id, this.createInitialRuntime(id));
    return server;
  }

  /** 用完整配置注册服务（id 已存在，用于 API 添加/加载配置） */
  registerServer(config: ServerConfig): void {
    this.servers.set(config.id, config);
    if (!this.runtimes.has(config.id)) {
      this.runtimes.set(config.id, this.createInitialRuntime(config.id));
    }
  }

  updateServer(id: string, patch: Partial<ServerConfig>): ServerConfig | undefined {
    const existing = this.servers.get(id);
    if (!existing) return undefined;
    const updated: ServerConfig = { ...existing, ...patch, updatedAt: new Date().toISOString() };
    this.servers.set(id, updated);
    return updated;
  }

  removeServer(id: string): boolean {
    if (this.runtimes.get(id)?.status === 'running') return false;
    this.servers.delete(id);
    this.runtimes.delete(id);
    this.transports.delete(id);
    return true;
  }

  async startServer(id: string, retryOnConflict = true): Promise<void> {
    const config = this.servers.get(id);
    if (!config) throw new Error(`Server ${id} not found`);
    const rt = this.runtimes.get(id);
    if (rt?.status === 'running') return;

    this.updateRuntime(id, { status: 'starting' });

    try {
      const transport = this.createTransport(config);
      this.setupTransportListeners(transport, id);
      await transport.start();
      this.transports.set(id, transport);
      this.updateRuntime(id, {
        status: 'running',
        startedAt: new Date().toISOString(),
        stoppedAt: undefined,
        error: undefined,
      });
      this.emit('log', `服务 ${config.name} 启动成功，端口 ${config.port}`);
      this.emit('server_started', id);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      const isPortConflict = msg.includes('EADDRINUSE') || msg.includes('address already in use');
      if (isPortConflict && retryOnConflict) {
        this.emit('log', `端口 ${config.port} 被占用，尝试释放...`);
        const killed = this.killPort(config.port);
        if (killed) {
          this.emit('log', `端口 ${config.port} 已释放，正在重试启动...`);
          await new Promise(r => setTimeout(r, 800));
          return this.startServer(id, false);
        }
        this.emit('log', `端口 ${config.port} 无法释放（无权限或 PID 不存在）`);
      }
      this.updateRuntime(id, { status: 'error', error: msg });
      this.emit('server_error', id, msg);
      throw err;
    }
  }

  async stopServer(id: string): Promise<void> {
    const transport = this.transports.get(id);
    if (!transport) return;

    this.updateRuntime(id, { status: 'stopping' });
    await transport.stop();
    this.transports.delete(id);
    this.updateRuntime(id, {
      status: 'stopped',
      stoppedAt: new Date().toISOString(),
    });
    this.emit('server_stopped', id);
  }

  async restartServer(id: string): Promise<void> {
    await this.stopServer(id);
    await this.startServer(id);
  }

  async startAll(): Promise<void> {
    const promises = Array.from(this.servers.keys()).map((id) =>
      this.startServer(id).catch((e) => this.emit('server_error', id, (e as Error).message))
    );
    await Promise.allSettled(promises);
  }

  async stopAll(): Promise<void> {
    await Promise.allSettled(
      Array.from(this.transports.keys()).map((id) => this.stopServer(id))
    );
  }

  async restartAll(): Promise<void> {
    await this.stopAll();
    await this.startAll();
  }

  getTransport(id: string): ITransport | undefined {
    return this.transports.get(id);
  }

  // =================== 私有方法 ===================

  /**
   * 尝试释放占用指定端口的进程
   * @returns true 表示有进程被终止，false 表示端口未被占用或无法终止
   */
  private killPort(port: number): boolean {
    if (process.platform !== 'win32') {
      // Unix: lsof + kill
      try {
        const { execFileSync } = require('child_process') as typeof import('child_process');
        const out = execFileSync('lsof', ['-ti', `:${port}`], {
          encoding: 'utf8',
          windowsHide: true,
        }) as string;
        if (!out.trim()) return false;
        for (const pid of out.trim().split('\n')) {
          try {
            execFileSync('kill', ['-9', pid.trim()], { windowsHide: true });
          } catch {}
        }
        return true;
      } catch {
        return false;
      }
    }

    // Windows: netstat + taskkill（写临时文件绕过 shell 编码问题）
    try {
      const { execFileSync } = require('child_process') as typeof import('child_process');
      const fs = require('fs') as typeof import('fs');
      const os = require('os') as typeof import('os');
      const tmpFile = `${os.tmpdir()}\\port_${port}_${Date.now()}.txt`;
      try { fs.unlinkSync(tmpFile); } catch {}
      execFileSync('cmd.exe', ['/c', `netstat -ano > "${tmpFile}"`], {
        windowsHide: true,
      });
      const content = fs.readFileSync(tmpFile, 'utf8') as string;
      try { fs.unlinkSync(tmpFile); } catch {}

      const pidSet = new Set<string>();
      for (const raw of content.split('\n')) {
        const line = raw.trim();
        if (!line.includes('LISTENING')) continue;
        if (!new RegExp(`:${port}[\\s\\t]`).test(line)) continue;
        const parts = line.split(/\s+/);
        const pid = parts[parts.length - 1];
        if (pid && /^\d+$/.test(pid)) pidSet.add(pid);
      }

      if (pidSet.size === 0) return false;

      let killed = false;
      for (const pid of pidSet) {
        try {
          execFileSync('taskkill', ['/PID', pid, '/F'], {
            windowsHide: true,
          });
          this.emit('log', {
            level: 'WARN',
            event: 'port_cleaned',
            message: `已释放端口 ${port}，终止进程 PID: ${pid}`,
          });
          killed = true;
        } catch {
          this.emit('log', {
            level: 'WARN',
            event: 'port_clean_failed',
            message: `无法终止进程 PID: ${pid}，可能已退出`,
          });
        }
      }

      if (killed) {
        // 等系统释放端口
        try {
          execFileSync('cmd.exe', ['/c', 'timeout /t 1 /nobreak >nul'], {
            windowsHide: true,
          });
        } catch {}
      }

      return killed;
    } catch {
      return false;
    }
  }

  private createInitialRuntime(serverId: string): ServerRuntime {
    return {
      id: serverId,
      status: 'stopped',
      clientCount: 0,
      totalConnections: 0,
      reconnectCount: 0,
      sentMessages: 0,
      receivedMessages: 0,
      sentBytes: 0,
      receivedBytes: 0,
    };
  }

  private updateRuntime(id: string, patch: Partial<ServerRuntime>): void {
    const existing = this.runtimes.get(id);
    if (existing) {
      this.runtimes.set(id, { ...existing, ...patch });
    }
  }

  private createTransport(config: ServerConfig): ITransport {
    if (config.protocol === 'websocket') {
      return new WebSocketTransport(config);
    }
    if (config.protocol === 'socket.io') {
      return new SocketIOTransport(config);
    }
    throw new Error(`Unsupported protocol: ${config.protocol}`);
  }

  private setupTransportListeners(transport: ITransport, serverId: string): void {
    (transport as unknown as EventEmitter).on('connect', (client: { id: string; serverId: string; socketId: string; ipAddress?: string }) => {
      this.updateRuntime(serverId, {
        clientCount: (this.runtimes.get(serverId)?.clientCount ?? 0) + 1,
        totalConnections: (this.runtimes.get(serverId)?.totalConnections ?? 0) + 1,
      });
      // 把 client 注册到 ClientManager（用 serverId___socketId 作为复合 id）
      const fullId = `${serverId}___${client.id}`;
      const clientInfo: import('../src/types/index.js').ClientInfo = {
        id: fullId,
        serverId,
        socketId: client.socketId ?? client.id,
        ipAddress: client.ipAddress ?? 'unknown',
        connectedAt: new Date().toISOString(),
        lastActivityAt: new Date().toISOString(),
        protocol: this.servers.get(serverId)?.protocol ?? 'websocket',
        status: 'connected',
      };
      this.clientManager.addClient(clientInfo);
      this.emit('client_connect', serverId, client.id);
    });

    transport.on('disconnect', (clientId: string) => {
      this.updateRuntime(serverId, {
        clientCount: Math.max(0, (this.runtimes.get(serverId)?.clientCount ?? 1) - 1),
      });
      this.clientManager.removeClient(`${serverId}___${clientId}`);
      this.emit('client_disconnect', serverId, clientId);
    });

    transport.on('message', (_clientId: string, _event: string, _data: unknown) => {
      this.updateRuntime(serverId, {
        receivedMessages: (this.runtimes.get(serverId)?.receivedMessages ?? 0) + 1,
      });
    });

    transport.on('error', (err: Error) => {
      this.emit('transport_error', serverId, err.message);
    });
  }
}

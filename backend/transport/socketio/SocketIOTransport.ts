/**
 * SocketIOTransport - Socket.IO 协议传输层实现
 * 实现 ITransport 接口
 */
import { Server as SocketIOServer, type ServerOptions } from 'socket.io';
import { createServer as createHttpServer } from 'http';
import { createServer as createHttpsServer } from 'https';
import { readFileSync } from 'fs';
import { nanoid } from 'nanoid';
import type { ServerConfig, ClientInfo, ProtocolType, HeartbeatConfig } from '../../src/types/index';
import { BaseTransport } from '../BaseTransport.js';

export class SocketIOTransport extends BaseTransport {
  readonly protocol: ProtocolType = 'socket.io';
  private io?: InstanceType<typeof SocketIOServer>;
  private httpServer?: ReturnType<typeof createHttpServer | typeof createHttpsServer>;
  private clientInfoMap = new Map<string, ClientInfo>();
  private isRunningFlag = false;
  private config!: ServerConfig;
  private heartbeatConfig?: HeartbeatConfig;

  constructor(config: ServerConfig, heartbeatConfig?: HeartbeatConfig) {
    super();
    this.config = config;
    this.heartbeatConfig = heartbeatConfig;
  }

  async start(): Promise<void> {
    if (this.isRunningFlag) return;

    return new Promise((resolve, reject) => {
      try {
        const { ip, port, wssEnabled, certPath, keyPath } = this.config;
        const hb = this.heartbeatConfig;

        let pingInterval: number | undefined;
        let pingTimeout: number | undefined;
        if (!hb || hb.enabled !== false) {
          pingInterval = hb?.pingInterval ?? 30000;
          pingTimeout = hb?.pongTimeout ?? 90000;
        }

        if (wssEnabled && certPath && keyPath) {
          const options = {
            cert: readFileSync(certPath),
            key: readFileSync(keyPath),
          };
          this.httpServer = createHttpsServer(options);
        } else {
          this.httpServer = createHttpServer();
        }

        const opts: any = {
          cors: { origin: '*', methods: ['GET', 'POST'] },
        };
        if (pingInterval !== undefined) opts.pingInterval = pingInterval;
        if (pingTimeout !== undefined) opts.pingTimeout = pingTimeout;

        this.io = new SocketIOServer(this.httpServer, opts);

        this.io.on('connection', (socket) => {
          const clientId = nanoid(16);

          const client: ClientInfo = {
            id: clientId,
            serverId: this.config.id,
            socketId: socket.id,
            ipAddress: socket.handshake.address,
            connectedAt: new Date().toISOString(),
            lastActivityAt: new Date().toISOString(),
            protocol: 'socket.io',
            status: 'connected',
          };

          this.clientInfoMap.set(socket.id, client);
          this.emitConnect(client);

          // 动态注册事件
          socket.onAny((event: string, ...args: unknown[]) => {
            client.lastActivityAt = new Date().toISOString();
            this.emitMessage(socket.id, event, args[0] ?? null);
          });

          socket.on('disconnect', (reason: string) => {
            this.clientInfoMap.delete(socket.id);
            this.emitDisconnect(socket.id, reason);
          });

          socket.on('error', (err: Error) => {
            this.emitError(err);
          });
        });

        this.httpServer.listen(port, ip, () => {
          this.isRunningFlag = true;
          this.info('socketio_start', `Socket.IO 服务启动成功 ${ip}:${port}`);
          resolve();
        });
      } catch (err) {
        reject(err as Error);
      }
    });
  }

  async stop(): Promise<void> {
    return new Promise((resolve) => {
      this.io?.close(() => {
        this.httpServer?.close(() => {
          this.isRunningFlag = false;
          this.clientInfoMap.clear();
          this.info('socketio_stop', 'Socket.IO 服务已停止');
          resolve();
        });
      });
    });
  }

  async send(clientId: string, event: string, data: unknown): Promise<void> {
    const socket = this.io?.sockets.sockets.get(clientId);
    if (socket) {
      socket.emit(event, data);
    }
  }

  async broadcast(event: string, data: unknown, targetIds?: string[]): Promise<void> {
    if (targetIds) {
      for (const id of targetIds) {
        await this.send(id, event, data);
      }
    } else {
      this.io?.emit(event, data);
    }
  }

  async disconnectClient(clientId: string): Promise<void> {
    const socket = this.io?.sockets.sockets.get(clientId);
    if (socket) {
      socket.disconnect(true);
      this.clientInfoMap.delete(clientId);
    }
  }

  getClients(): ClientInfo[] {
    return Array.from(this.clientInfoMap.values());
  }

  isRunning(): boolean {
    return this.isRunningFlag;
  }

  /** 获取底层 Socket.IO Server 实例（供 EventManager 动态注册事件） */
  getIOServer(): InstanceType<typeof SocketIOServer> | undefined {
    return this.io;
  }
}

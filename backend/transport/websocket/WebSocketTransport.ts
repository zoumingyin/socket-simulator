/**
 * WebSocketTransport - WebSocket 协议传输层实现
 * 实现 ITransport 接口，支持 ws:// 和 wss://
 */
import { WebSocketServer, type WebSocket } from 'ws';
import type { IncomingMessage } from 'http';
import { createServer as createHttpsServer } from 'https';
import { createServer as createHttpServer } from 'http';
import { readFileSync } from 'fs';
import { nanoid } from 'nanoid';
import type { ServerConfig, ClientInfo, ProtocolType, TransportEvents } from '../../src/types/index';
import { BaseTransport } from '../BaseTransport.js';

export class WebSocketTransport extends BaseTransport {
  readonly protocol: ProtocolType = 'websocket';
  private server?: InstanceType<typeof WebSocketServer>;
  private httpServer?: ReturnType<typeof createHttpServer | typeof createHttpsServer>;
  private clients = new Map<string, WebSocket>();
  private config!: ServerConfig;
  private isRunningFlag = false;

  constructor(config: ServerConfig) {
    super();
    this.config = config;
  }

  async start(): Promise<void> {
    if (this.isRunningFlag) return;

    return new Promise((resolve, reject) => {
      try {
        const { ip, port, wssEnabled, certPath, keyPath } = this.config;

        if (wssEnabled && certPath && keyPath) {
          const options = {
            cert: readFileSync(certPath),
            key: readFileSync(keyPath),
          };
          this.httpServer = createHttpsServer(options);
        } else {
          this.httpServer = createHttpServer();
        }

        this.server = new WebSocketServer({ server: this.httpServer, path: '/' });

        this.server.on('connection', (ws: WebSocket, req: IncomingMessage) => {
          const clientId = nanoid(16);
          this.clients.set(clientId, ws);

          const client: ClientInfo = {
            id: clientId,
            serverId: this.config.id,
            socketId: clientId,
            ipAddress: req.socket.remoteAddress ?? 'unknown',
            connectedAt: new Date().toISOString(),
            lastActivityAt: new Date().toISOString(),
            protocol: 'websocket',
            status: 'connected',
          };

          this.emitConnect(client);

          ws.on('message', (data: Buffer) => {
            try {
              const parsed = JSON.parse(data.toString()) as { event: string; data: Record<string, unknown> };
              client.lastActivityAt = new Date().toISOString();
              this.emitMessage(clientId, parsed.event, parsed.data);
            } catch {
              // 非 JSON 文本消息
              this.emitMessage(clientId, 'message', { raw: data.toString() });
            }
          });

          ws.on('close', () => {
            this.clients.delete(clientId);
            this.emitDisconnect(clientId, 'client closed');
          });

          ws.on('error', (err: Error) => {
            this.emitError(err);
          });
        });

        this.server.on('error', (err: Error) => {
          this.emitError(err);
          reject(err);
        });

        this.httpServer.listen(port, ip, () => {
          this.isRunningFlag = true;
          this.info('websocket_start', `WebSocket 服务启动成功 ${ip}:${port}`);
          resolve();
        });
      } catch (err) {
        reject(err as Error);
      }
    });
  }

  async stop(): Promise<void> {
    return new Promise((resolve) => {
      this.clients.forEach((ws) => ws.close());
      this.clients.clear();

      this.server?.close(() => {
        this.httpServer?.close(() => {
          this.isRunningFlag = false;
          this.info('websocket_stop', 'WebSocket 服务已停止');
          resolve();
        });
      });
    });
  }

  async send(clientId: string, event: string, data: unknown): Promise<void> {
    const ws = this.clients.get(clientId);
    if (!ws || ws.readyState !== ws.OPEN) return;

    const message = JSON.stringify({ event, data });
    ws.send(message);
  }

  async broadcast(event: string, data: unknown, targetIds?: string[]): Promise<void> {
    const message = JSON.stringify({ event, data });

    if (targetIds) {
      for (const id of targetIds) {
        await this.send(id, event, data);
      }
    } else {
      this.clients.forEach((ws) => {
        if (ws.readyState === ws.OPEN) {
          ws.send(message);
        }
      });
    }
  }

  async disconnectClient(clientId: string): Promise<void> {
    const ws = this.clients.get(clientId);
    if (ws) {
      ws.close();
      this.clients.delete(clientId);
    }
  }

  getClients(): ClientInfo[] {
    // 返回快照（实际应从 ClientManager 获取完整信息）
    return [];
  }

  isRunning(): boolean {
    return this.isRunningFlag;
  }
}

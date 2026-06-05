/**
 * ITransport - 传输层抽象接口（协议插件化设计）
 * 所有传输协议（WebSocket、Socket.IO、未来 TCP/UDP/MQTT）必须实现此接口
 */
import { EventEmitter } from 'events';
import type {
  ITransport,
  ClientInfo,
  ProtocolType,
  TransportEvents,
} from '../src/types/index';

export abstract class BaseTransport extends EventEmitter implements ITransport {
  abstract readonly protocol: ProtocolType;

  abstract start(): Promise<void>;
  abstract stop(): Promise<void>;
  abstract send(clientId: string, event: string, data: unknown): Promise<void>;
  abstract broadcast(event: string, data: unknown, targetIds?: string[]): Promise<void>;
  abstract disconnectClient(clientId: string): Promise<void>;
  abstract getClients(): ClientInfo[];
  abstract isRunning(): boolean;

  protected emitConnect(client: ClientInfo): void {
    this.emit('connect', client);
  }

  protected emitDisconnect(clientId: string, reason?: string): void {
    this.emit('disconnect', clientId, reason);
  }

  protected emitMessage(clientId: string, event: string, data: unknown): void {
    this.emit('message', clientId, event, data);
  }

  protected emitError(error: Error): void {
    this.emit('error', error);
  }

  /** 日志记录（供子类调用，通过 EventEmitter 发出 log 事件） */
  protected info(event: string, message: string, serverId?: string): void {
    this.emit('log', { level: 'INFO', event, message, serverId });
  }

  protected warn(event: string, message: string, serverId?: string): void {
    this.emit('log', { level: 'WARN', event, message, serverId });
  }

  protected error(event: string, message: string, serverId?: string): void {
    this.emit('log', { level: 'ERROR', event, message, serverId });
  }
}

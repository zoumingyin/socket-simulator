/**
 * BaseManager - 所有 Manager 的基类
 * 提供通用日志、事件发射、错误处理能力
 */
import { EventEmitter } from 'events';
import type { LogLevel, LogEntry } from '../src/types/index';

export abstract class BaseManager extends EventEmitter {
  protected readonly serviceName: string;

  protected constructor(serviceName: string) {
    super();
    this.serviceName = serviceName;
  }

  /** 统一日志输出 */
  protected log(level: LogLevel, event: string, message: string, meta?: Record<string, unknown>): void {
    const entry: LogEntry = {
      id: crypto.randomUUID(),
      level,
      event,
      message: `[${this.serviceName}] ${message}`,
      timestamp: new Date().toISOString(),
      ...(meta && { metadata: meta }),
    };
    this.emit('log', entry);
  }

  protected info(event: string, message: string, meta?: Record<string, unknown>): void {
    this.log('INFO', event, message, meta);
  }

  protected warn(event: string, message: string, meta?: Record<string, unknown>): void {
    this.log('WARN', event, event, meta);
  }

  protected error(event: string, message: string, meta?: Record<string, unknown>): void {
    this.log('ERROR', event, message, meta);
  }

  protected debug(event: string, message: string, meta?: Record<string, unknown>): void {
    this.log('DEBUG', event, message, meta);
  }

  /** 抽象方法：启动 */
  abstract start(): Promise<void>;

  /** 抽象方法：停止 */
  abstract stop(): Promise<void>;
}

/**
 * LogManager - 日志系统管理
 * 实时收集日志、按条件过滤、导出/清空日志文件
 */
import { EventEmitter } from 'events';
import { appendFileSync, readFileSync, writeFileSync, existsSync, mkdirSync } from 'fs';
import { join } from 'path';
import type { LogEntry, LogLevel, LogFilter } from '../src/types/index';

// 不用 import.meta.url + fileURLToPath（Windows 路径含 \n 等会被解码成换行符）
// 改用 process.cwd()，假定后端从 backend/ 目录启动
const logDir = join(process.cwd(), '../logs');

export class LogManager extends EventEmitter {
  private entries: LogEntry[] = [];
  private maxMemoryEntries = 2000;

  constructor() {
    super();
    if (!existsSync(logDir)) {
      mkdirSync(logDir, { recursive: true });
    }
  }

  /** 添加日志条目 */
  addEntry(entry: LogEntry): void {
    this.entries.push(entry);
    if (this.entries.length > this.maxMemoryEntries) {
      this.entries = this.entries.slice(-this.maxMemoryEntries);
    }
    this.writeToFile(entry);
    this.emit('log', entry);
  }

  /** 获取内存中的日志（支持过滤） */
  getEntries(filter?: LogFilter): LogEntry[] {
    let result = [...this.entries];
    if (filter?.serverId) {
      result = result.filter(e => e.serverId === filter.serverId);
    }
    if (filter?.level) {
      const levelOrder = ['DEBUG', 'INFO', 'WARN', 'ERROR'];
      const minIdx = levelOrder.indexOf(filter.level);
      result = result.filter(e => levelOrder.indexOf(e.level) >= minIdx);
    }
    if (filter?.keyword) {
      const kw = filter.keyword.toLowerCase();
      result = result.filter(e =>
        e.message.toLowerCase().includes(kw) ||
        (e.serverId && e.serverId.toLowerCase().includes(kw))
      );
    }
    return result;
  }

  /** 清空内存日志 */
  clearEntries(): void {
    this.entries = [];
  }

  /** 导出日志到 JSON 文件 */
  exportToFile(filePath: string): void {
    const data = JSON.stringify(this.entries, null, 2);
    writeFileSync(filePath, data, 'utf-8');
  }

  /** 从文件导入日志 */
  importFromFile(filePath: string): void {
    const raw = readFileSync(filePath, 'utf-8');
    const imported = JSON.parse(raw) as LogEntry[];
    this.entries = imported.slice(-this.maxMemoryEntries);
  }

  /** 写入日志文件（按日期分文件） */
  private writeToFile(entry: LogEntry): void {
    try {
      const d = new Date(entry.timestamp);
      const dateStr = isNaN(d.getTime())
        ? new Date().toISOString().split('T')[0]
        : entry.timestamp.split('T')[0];
      const file = join(logDir, `${dateStr}.log`);
      const line = JSON.stringify(entry) + '\n';
      appendFileSync(file, line, 'utf-8');
    } catch {
      // 写入失败时静默忽略，不影响主流程
    }
  }
}

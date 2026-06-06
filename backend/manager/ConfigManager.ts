/**
 * ConfigManager - 配置持久化管理
 * 负责 config.json（统一存储所有配置）的读写
 * 使用纯 fs 实现，避免 steno (lowdb) 在 Windows 上的 EBADF 问题
 */
import fs from 'fs';
import path from 'path';
import type {
  ServerConfig,
  EventConfig,
  SystemSettings,
  WindowConfig,
  PersistedConfig,
} from '../src/types/index';

// 不用 import.meta.url + fileURLToPath（Windows 路径含 \n 等会被解码成换行符）
// 改用 process.cwd()，假定后端从 backend/ 目录启动
const configDir = path.resolve(process.cwd(), '../config');
const configFile = path.join(configDir, 'config.json');

interface DBData extends PersistedConfig {}

export class ConfigManager {
  private getDefaultData(): DBData {
    return {
      servers: [],
      events: [],
      systemSettings: this.getDefaultSystemSettings(),
      windowConfig: this.getDefaultWindowConfig(),
      version: '1.0.0',
      exportedAt: new Date().toISOString(),
    };
  }

  private getDefaultSystemSettings(): SystemSettings {
    return {
      id: 'system',
      heartbeat: { enabled: true, pingInterval: 30000, pongTimeout: 90000 },
      wss: { enabled: false, certPath: '', keyPath: '' },
      ipAccess: { whitelist: [], blacklist: [] },
      autoStart: false,
      startMinimized: false,
      logRetentionDays: 7,
      maxConnectionsPerServer: 1000,
      updatedAt: new Date().toISOString(),
    };
  }

  private getDefaultWindowConfig(): WindowConfig {
    return { width: 1280, height: 800, maximized: false };
  }

  /** 读取 config.json，不存在则返回默认数据（含数据修正） */
  private readData(): DBData {
    try {
      if (!fs.existsSync(configFile)) return this.getDefaultData();
      const raw = fs.readFileSync(configFile, 'utf-8');
      const parsed = JSON.parse(raw) as DBData;
      return this.sanitizeData(parsed);
    } catch {
      return this.getDefaultData();
    }
  }

  /** 防御性修正：防止旧数据中越界/异常值污染运行时 */
  private sanitizeData(data: DBData): DBData {
    if (!data.systemSettings) return data;

    const s = data.systemSettings;
    const defaults = this.getDefaultSystemSettings();

    // 心跳参数合法范围
    if (s.heartbeat) {
      s.heartbeat.pingInterval = this.clamp(
        Number(s.heartbeat.pingInterval) || defaults.heartbeat.pingInterval,
        5000, 300000,
      );
      s.heartbeat.pongTimeout = this.clamp(
        Number(s.heartbeat.pongTimeout) || defaults.heartbeat.pongTimeout,
        10000, 600000,
      );
    }

    // 日志保留天数
    s.logRetentionDays = this.clamp(
      Number(s.logRetentionDays) || defaults.logRetentionDays,
      1, 365,
    );

    // 最大连接数
    s.maxConnectionsPerServer = this.clamp(
      Number(s.maxConnectionsPerServer) || defaults.maxConnectionsPerServer,
      1, 10000,
    );

    return data;
  }

  private clamp(value: number, min: number, max: number): number {
    return Math.max(min, Math.min(max, value));
  }

  /** 写入 config.json */
  private writeData(data: DBData): void {
    if (!fs.existsSync(configDir)) {
      fs.mkdirSync(configDir, { recursive: true });
    }
    fs.writeFileSync(configFile, JSON.stringify(data, null, 2), 'utf-8');
  }

  // ==================== 初始化 ====================

  init(): void {
    // 确保目录存在，读取一次触发创建默认文件
    const data = this.readData();
    this.writeData(data);
  }

  // ==================== ServerConfig ====================

  getServers(): ServerConfig[] {
    return this.readData().servers ?? [];
  }

  saveServers(servers: ServerConfig[]): void {
    const data = this.readData();
    data.servers = servers;
    data.exportedAt = new Date().toISOString();
    this.writeData(data);
  }

  getServerById(id: string): ServerConfig | undefined {
    return this.getServers().find((s) => s.id === id);
  }

  // ==================== EventConfig ====================

  getEvents(): EventConfig[] {
    return this.readData().events ?? [];
  }

  saveEvents(events: EventConfig[]): void {
    const data = this.readData();
    data.events = events;
    data.exportedAt = new Date().toISOString();
    this.writeData(data);
  }

  // ==================== SystemSettings ====================

  getSystemSettings(): SystemSettings {
    return this.readData().systemSettings ?? this.getDefaultSystemSettings();
  }

  saveSystemSettings(settings: SystemSettings): void {
    const data = this.readData();
    const defaults = this.getDefaultSystemSettings();
    // 确保数值字段为 number 类型且在合法范围内
    data.systemSettings = {
      ...settings,
      heartbeat: settings.heartbeat ? {
        enabled: settings.heartbeat.enabled,
        pingInterval: this.clamp(
          Number(settings.heartbeat.pingInterval) || defaults.heartbeat.pingInterval,
          5000, 300000,
        ),
        pongTimeout: this.clamp(
          Number(settings.heartbeat.pongTimeout) || defaults.heartbeat.pongTimeout,
          10000, 600000,
        ),
      } : settings.heartbeat,
      logRetentionDays: this.clamp(
        Number(settings.logRetentionDays) || defaults.logRetentionDays,
        1, 365,
      ),
      maxConnectionsPerServer: this.clamp(
        Number(settings.maxConnectionsPerServer) || defaults.maxConnectionsPerServer,
        1, 10000,
      ),
      updatedAt: new Date().toISOString(),
    };
    data.exportedAt = new Date().toISOString();
    this.writeData(data);
  }

  // ==================== WindowConfig ====================

  getWindowConfig(): WindowConfig {
    return this.readData().windowConfig ?? this.getDefaultWindowConfig();
  }

  saveWindowConfig(config: WindowConfig): void {
    const data = this.readData();
    data.windowConfig = config;
    data.exportedAt = new Date().toISOString();
    this.writeData(data);
  }

  // ==================== 导入 / 导出 ====================

  exportAll(): PersistedConfig {
    const data = this.readData();
    return {
      servers: data.servers,
      events: data.events,
      systemSettings: data.systemSettings,
      windowConfig: data.windowConfig,
      version: data.version,
      exportedAt: new Date().toISOString(),
    };
  }

  importAll(config: PersistedConfig): void {
    const data = this.readData();
    data.servers = config.servers;
    data.events = config.events;
    data.systemSettings = config.systemSettings;
    data.windowConfig = config.windowConfig;
    data.version = config.version;
    data.exportedAt = new Date().toISOString();
    this.writeData(data);
  }
}

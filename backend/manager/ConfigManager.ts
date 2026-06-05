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
  MessageTemplate,
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
      templates: [],
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

  /** 读取 config.json，不存在则返回默认数据 */
  private readData(): DBData {
    try {
      if (!fs.existsSync(configFile)) return this.getDefaultData();
      const raw = fs.readFileSync(configFile, 'utf-8');
      return JSON.parse(raw) as DBData;
    } catch {
      return this.getDefaultData();
    }
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

  // ==================== MessageTemplate ====================

  getTemplates(): MessageTemplate[] {
    return this.readData().templates ?? [];
  }

  saveTemplates(templates: MessageTemplate[]): void {
    const data = this.readData();
    data.templates = templates;
    data.exportedAt = new Date().toISOString();
    this.writeData(data);
  }

  // ==================== SystemSettings ====================

  getSystemSettings(): SystemSettings {
    return this.readData().systemSettings ?? this.getDefaultSystemSettings();
  }

  saveSystemSettings(settings: SystemSettings): void {
    const data = this.readData();
    data.systemSettings = { ...settings, updatedAt: new Date().toISOString() };
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
      templates: data.templates,
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
    data.templates = config.templates;
    data.systemSettings = config.systemSettings;
    data.windowConfig = config.windowConfig;
    data.version = config.version;
    data.exportedAt = new Date().toISOString();
    this.writeData(data);
  }
}

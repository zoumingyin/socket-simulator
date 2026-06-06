/**
 * api/index.ts - REST API 完整实现
 * 覆盖前端所有 Zustand store 调用的路由
 */
import { createServer, IncomingMessage, ServerResponse } from 'http';
import { parse } from 'url';
import { nanoid } from 'nanoid';
import type {
  ServerConfig,
  ServerRuntime,
  EventConfig,
  ClientInfo,
  ClientGroup,
  MessageTemplate,
  SendMessageRequest,
  LogFilter,
  LogEntry,
  LogLevel,
  SystemSettings,
  WindowConfig,
  ApiResponse,
  PersistedConfig,
  ProtocolType,
} from '../src/types/index';
import { getApp } from '../main.js';
import { Server as SocketIOServer } from 'socket.io';

const PORT = 3080;

// ==================== 工具函数 ====================

function readBody<T>(req: IncomingMessage): Promise<T> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    req.on('data', chunk => chunks.push(chunk));
    req.on('end', () => {
      try { resolve(JSON.parse(Buffer.concat(chunks).toString('utf8'))); }
      catch (e) { reject(e); }
    });
    req.on('error', reject);
  });
}

function sendJSON(res: ServerResponse, status: number, data: unknown): void {
  res.writeHead(status, {
    'Content-Type': 'application/json; charset=utf-8',
    'Access-Control-Allow-Origin': '*',
    'Access-Control-Allow-Methods': 'GET, POST, PUT, DELETE, OPTIONS',
    'Access-Control-Allow-Headers': 'Content-Type',
  });
  res.end(JSON.stringify(data));
}

function getQueryParams(req: IncomingMessage): Record<string, string> {
  const parsed = parse(req.url ?? '', true);
  return (parsed.query as Record<string, string>) ?? {};
}

// ==================== 路由处理 ====================

async function handleRequest(req: IncomingMessage, res: ServerResponse): Promise<void> {
  // CORS preflight
  if (req.method === 'OPTIONS') {
    res.writeHead(204, {
      'Access-Control-Allow-Origin': '*',
      'Access-Control-Allow-Methods': 'GET, POST, PUT, DELETE, OPTIONS',
      'Access-Control-Allow-Headers': 'Content-Type',
    });
    res.end();
    return;
  }

  const method = req.method ?? 'GET';
  const parsedUrl = parse(req.url ?? '', true);
  let pathname = (parsedUrl.pathname ?? '').replace(/\/+$/, '') || '/';
  const query = (parsedUrl.query ?? {}) as Record<string, string>;

  // 兼容前端无前缀的调用（/server/list → /api/server/list）
  if (!pathname.startsWith('/api/')) {
    pathname = '/api' + pathname;
  }

  try {
    // ============ 服务管理 ============

    // GET /api/servers  (合并接口)
    if (method === 'GET' && pathname === '/api/servers') {
      const app = await getApp();
      const configs = app.configManager.getServers();
      const runtimes = app.serviceManager.getAllRuntimes();
      return sendJSON(res, 200, { success: true, data: { configs, runtimes } });
    }

    // GET /api/server/list
    if (method === 'GET' && pathname === '/api/server/list') {
      const app = await getApp();
      const configs = app.configManager.getServers();
      return sendJSON(res, 200, { success: true, data: configs });
    }

    // GET /api/server/runtimes
    if (method === 'GET' && pathname === '/api/server/runtimes') {
      const app = await getApp();
      const result: Record<string, ServerRuntime> = {};
      for (const rt of app.serviceManager.getAllRuntimes()) {
        result[rt.id] = rt;
      }
      return sendJSON(res, 200, { success: true, data: result });
    }

    // POST /api/server/add
    if (method === 'POST' && pathname === '/api/server/add') {
      const body = await readBody<ServerConfig>(req);
      const app = await getApp();
      // 确保有 id 和时间戳
      if (!body.id) body.id = nanoid();
      if (!body.createdAt) body.createdAt = new Date().toISOString();
      if (!body.updatedAt) body.updatedAt = new Date().toISOString();
      // 注册到 ServiceManager（运行时 Map）
      app.serviceManager.registerServer(body);
      // 持久化到 ConfigManager
      const servers = app.configManager.getServers();
      servers.push(body);
      app.configManager.saveServers(servers);
      return sendJSON(res, 200, { success: true, data: body, message: '添加成功' });
    }

    // POST /api/server/update
    if (method === 'POST' && pathname === '/api/server/update') {
      const body = await readBody<ServerConfig>(req);
      const app = await getApp();
      // 同步到 ServiceManager
      app.serviceManager.updateServer(body.id, body);
      // 持久化到 ConfigManager
      const servers = app.configManager.getServers();
      const idx = servers.findIndex(s => s.id === body.id);
      if (idx === -1) return sendJSON(res, 404, { success: false, error: '服务不存在' });
      servers[idx] = body;
      app.configManager.saveServers(servers);
      return sendJSON(res, 200, { success: true, message: '更新成功' });
    }

    // POST /api/server/remove
    if (method === 'POST' && pathname === '/api/server/remove') {
      const body = await readBody<{ id: string }>(req);
      const app = await getApp();
      // 先从 ServiceManager 移除（会检查运行状态）
      const removed = app.serviceManager.removeServer(body.id);
      if (!removed) return sendJSON(res, 400, { success: false, error: '服务正在运行，无法删除' });
      // 从 ConfigManager 持久化中移除
      let servers = app.configManager.getServers();
      servers = servers.filter(s => s.id !== body.id);
      app.configManager.saveServers(servers);
      return sendJSON(res, 200, { success: true, message: '删除成功' });
    }

    // POST /api/server/start
    if (method === 'POST' && pathname === '/api/server/start') {
      const body = await readBody<{ id: string }>(req);
      const app = await getApp();
      await app.serviceManager.startServer(body.id);
      return sendJSON(res, 200, { success: true, message: '启动成功' });
    }

    // POST /api/server/stop
    if (method === 'POST' && pathname === '/api/server/stop') {
      const body = await readBody<{ id: string }>(req);
      const app = await getApp();
      await app.serviceManager.stopServer(body.id);
      return sendJSON(res, 200, { success: true, message: '停止成功' });
    }

    // POST /api/server/restart
    if (method === 'POST' && pathname === '/api/server/restart') {
      const body = await readBody<{ id: string }>(req);
      const app = await getApp();
      await app.serviceManager.restartServer(body.id);
      return sendJSON(res, 200, { success: true, message: '重启成功' });
    }

    // POST /api/server/start-all
    if (method === 'POST' && pathname === '/api/server/start-all') {
      const app = await getApp();
      await app.serviceManager.startAll();
      return sendJSON(res, 200, { success: true, message: '全部启动成功' });
    }

    // POST /api/server/stop-all
    if (method === 'POST' && pathname === '/api/server/stop-all') {
      const app = await getApp();
      await app.serviceManager.stopAll();
      return sendJSON(res, 200, { success: true, message: '全部停止成功' });
    }

    // POST /api/server/restart-all
    if (method === 'POST' && pathname === '/api/server/restart-all') {
      const app = await getApp();
      await app.serviceManager.restartAll();
      return sendJSON(res, 200, { success: true, message: '全部重启成功' });
    }

    // ============ 事件管理 ============

    // GET /api/events
    if (method === 'GET' && pathname === '/api/events') {
      const app = await getApp();
      const serverId = query.serverId ?? '';
      let events: EventConfig[] = app.configManager.getEvents();
      if (serverId) events = events.filter(e => e.serverId === serverId);
      return sendJSON(res, 200, { success: true, data: events });
    }

    // POST /api/events/add
    if (method === 'POST' && pathname === '/api/events/add') {
      const body = await readBody<EventConfig>(req);
      const app = await getApp();
      // 确保有 id 和时间戳
      if (!body.id) body.id = nanoid();
      if (!body.createdAt) body.createdAt = new Date().toISOString();
      if (!body.updatedAt) body.updatedAt = new Date().toISOString();
      const evt = app.eventManager.addEvent(body);
      // 持久化
      const events = app.configManager.getEvents();
      events.push(evt);
      app.configManager.saveEvents(events);
      return sendJSON(res, 200, { success: true, data: evt, message: '事件添加成功' });
    }

    // POST /api/events/update
    if (method === 'POST' && pathname === '/api/events/update') {
      const body = await readBody<EventConfig>(req);
      const app = await getApp();
      const evt = app.eventManager.updateEvent(body.id, body);
      if (!evt) return sendJSON(res, 404, { success: false, error: '事件不存在' });
      // 持久化
      const events = app.configManager.getEvents();
      const idx = events.findIndex(e => e.id === body.id);
      if (idx !== -1) events[idx] = evt;
      app.configManager.saveEvents(events);
      return sendJSON(res, 200, { success: true, data: evt, message: '事件更新成功' });
    }

    // POST /api/events/remove
    if (method === 'POST' && pathname === '/api/events/remove') {
      const body = await readBody<{ id: string }>(req);
      const app = await getApp();
      const removed = app.eventManager.removeEvent(body.id);
      if (!removed) return sendJSON(res, 404, { success: false, error: '事件不存在' });
      // 持久化
      let events = app.configManager.getEvents();
      events = events.filter(e => e.id !== body.id);
      app.configManager.saveEvents(events);
      return sendJSON(res, 200, { success: true, message: '事件删除成功' });
    }

    // POST /api/events/toggle
    if (method === 'POST' && pathname === '/api/events/toggle') {
      const body = await readBody<{ id: string; status: 'enabled' | 'disabled' }>(req);
      const app = await getApp();
      const evt = app.eventManager.toggleEvent(body.id, body.status);
      if (!evt) return sendJSON(res, 404, { success: false, error: '事件不存在' });
      // 持久化
      const events = app.configManager.getEvents();
      const idx = events.findIndex(e => e.id === body.id);
      if (idx !== -1) events[idx] = evt;
      app.configManager.saveEvents(events);
      return sendJSON(res, 200, { success: true, data: evt, message: '状态切换成功' });
    }

    // ============ 客户端管理 ============

    // GET /api/clients
    if (method === 'GET' && pathname === '/api/clients') {
      const app = await getApp();
      const serverId = query.serverId ?? '';
      const clients = serverId
        ? app.clientManager.getClients(serverId)
        : app.clientManager.getClients();
      return sendJSON(res, 200, { success: true, data: clients });
    }

    // POST /api/client/disconnect
    if (method === 'POST' && pathname === '/api/client/disconnect') {
      const body = await readBody<{ clientId: string }>(req);
      const app = await getApp();
      // clientId format: serverId___clientId
      const [serverId, ...rest] = body.clientId.split('___');
      const actualClientId = rest.join('___');
      await app.clientManager.disconnectClient(serverId, actualClientId || body.clientId);
      return sendJSON(res, 200, { success: true, message: '客户端已断开' });
    }

    // POST /api/client/send  (向前端广播/指定客户端发消息)
    if (method === 'POST' && pathname === '/api/client/send') {
      const body = await readBody<{
        serverId: string;
        targetType?: 'broadcast' | 'client' | 'group';
        targetId?: string;
        event: string;
        messageType?: string;
        content?: string;
        data?: unknown;
        clientId?: string;
      }>(req);
      const app = await getApp();
      const serverId = body.serverId || (body.clientId ? body.clientId.split('___')[0] : '');
      const transport = app.serviceManager.getTransport(serverId);
      if (!transport) return sendJSON(res, 400, { success: false, error: `服务 ${serverId} 未运行` });

      const data = body.data ?? (body.content ? (body.messageType === 'json' ? JSON.parse(body.content) : body.content) : {});
      if (body.targetType === 'broadcast' || !body.targetId) {
        await transport.broadcast(body.event, data);
        app.serviceManager.incrementSentMessages(serverId);
        try {
          app.logManager.addEntry({
            id: nanoid(),
            timestamp: new Date().toISOString(),
            level: 'INFO',
            event: body.event,
            serverId: serverId,
            message: `[消息中心] 广播消息 → 事件: ${body.event}, 内容: ${typeof data === 'string' ? data : JSON.stringify(data)}`,
            metadata: { targetType: 'broadcast', targetId: undefined, event: body.event, content: typeof data === 'string' ? data : JSON.stringify(data) },
          });
          console.log(`[Log] Broadcast log entry added: ${body.event}`);
        } catch (err) {
          console.error('[Log] Failed to add broadcast log entry:', err);
        }
      } else {
        // targetId 格式可能是 "serverId___socketId"，需要提取真正的 socketId
        const socketId = body.targetId.includes('___')
          ? body.targetId.split('___').slice(1).join('___')
          : body.targetId;
        await transport.send(socketId, body.event, data);
        app.serviceManager.incrementSentMessages(serverId);
        try {
          app.logManager.addEntry({
            id: nanoid(),
            timestamp: new Date().toISOString(),
            level: 'INFO',
            event: body.event,
            serverId: serverId,
            clientId: body.targetId,
            message: `[消息中心] 指定发送 → 客户端: ${body.targetId}, 事件: ${body.event}, 内容: ${typeof data === 'string' ? data : JSON.stringify(data)}`,
            metadata: { targetType: body.targetType, targetId: body.targetId, event: body.event, content: typeof data === 'string' ? data : JSON.stringify(data) },
          });
          console.log(`[Log] Targeted log entry added: ${body.event} -> ${body.targetId}`);
        } catch (err) {
          console.error('[Log] Failed to add targeted log entry:', err);
        }
      }
      return sendJSON(res, 200, { success: true, message: '消息已发送' });
    }

    // POST /api/send-message  (消息中心"指定客户端"发送)
    if (method === 'POST' && pathname === '/api/send-message') {
      const body = await readBody<{
        serverId: string;
        targetType: 'broadcast' | 'client' | 'group';
        targetId?: string;
        event: string;
        messageType?: string;
        content?: string;
      }>(req);
      const app = await getApp();
      const transport = app.serviceManager.getTransport(body.serverId);
      if (!transport) return sendJSON(res, 400, { success: false, error: `服务 ${body.serverId} 未运行` });

      const data = body.content ? (body.messageType === 'json' ? JSON.parse(body.content) : body.content) : {};
      if (body.targetType === 'broadcast') {
        await transport.broadcast(body.event, data);
        app.serviceManager.incrementSentMessages(body.serverId);
        try {
          app.logManager.addEntry({
            id: nanoid(),
            timestamp: new Date().toISOString(),
            level: 'INFO',
            event: body.event,
            serverId: body.serverId,
            message: `[消息中心] 广播消息 → 事件: ${body.event}, 内容: ${typeof data === 'string' ? data : JSON.stringify(data)}`,
            metadata: { targetType: 'broadcast', targetId: undefined, event: body.event, content: typeof data === 'string' ? data : JSON.stringify(data) },
          });
          console.log(`[Log] Send-message broadcast log entry added: ${body.event}`);
        } catch (err) {
          console.error('[Log] Failed to add send-message broadcast log entry:', err);
        }
      } else if (body.targetId) {
        // targetId 格式可能是 "serverId___socketId"，需要提取真正的 socketId
        const socketId = body.targetId.includes('___')
          ? body.targetId.split('___').slice(1).join('___')
          : body.targetId;
        await transport.send(socketId, body.event, data);
        app.serviceManager.incrementSentMessages(body.serverId);
        try {
          app.logManager.addEntry({
            id: nanoid(),
            timestamp: new Date().toISOString(),
            level: 'INFO',
            event: body.event,
            serverId: body.serverId,
            clientId: body.targetId,
            message: `[消息中心] 指定发送 → 客户端: ${body.targetId}, 事件: ${body.event}, 内容: ${typeof data === 'string' ? data : JSON.stringify(data)}`,
            metadata: { targetType: body.targetType, targetId: body.targetId, event: body.event, content: typeof data === 'string' ? data : JSON.stringify(data) },
          });
          console.log(`[Log] Send-message targeted log entry added: ${body.event} -> ${body.targetId}`);
        } catch (err) {
          console.error('[Log] Failed to add send-message targeted log entry:', err);
        }
      }
      return sendJSON(res, 200, { success: true, message: '消息已发送' });
    }

    // ============ 消息模板 ============

    // GET /api/templates
    if (method === 'GET' && pathname === '/api/templates') {
      const app = await getApp();
      const templates: MessageTemplate[] = app.configManager.getTemplates();
      return sendJSON(res, 200, { success: true, data: templates });
    }

    // POST /api/template/save
    if (method === 'POST' && pathname === '/api/template/save') {
      const body = await readBody<MessageTemplate>(req);
      const app = await getApp();
      const templates = app.configManager.getTemplates();
      const idx = templates.findIndex(t => t.id === body.id);
      if (idx >= 0) templates[idx] = body;
      else templates.push(body);
      app.configManager.saveTemplates(templates);
      return sendJSON(res, 200, { success: true, message: '模板保存成功' });
    }

    // POST /api/template/delete
    if (method === 'POST' && pathname === '/api/template/delete') {
      const body = await readBody<{ id: string }>(req);
      const app = await getApp();
      let templates = app.configManager.getTemplates();
      templates = templates.filter(t => t.id !== body.id);
      app.configManager.saveTemplates(templates);
      return sendJSON(res, 200, { success: true, message: '模板删除成功' });
    }

    // ============ 日志 ============

    // GET /api/logs
    if (method === 'GET' && pathname === '/api/logs') {
      const app = await getApp();
      const filter: LogFilter = {
        serverId: query.serverId,
        level: query.level as LogLevel || undefined,
        keyword: query.keyword,
      };
      const entries: LogEntry[] = app.logManager.getEntries(filter);
      return sendJSON(res, 200, { success: true, data: entries });
    }

    // POST /api/logs/clear
    if (method === 'POST' && pathname === '/api/logs/clear') {
      const app = await getApp();
      app.logManager.clearEntries();
      return sendJSON(res, 200, { success: true, message: '日志已清空' });
    }

    // ============ 系统设置 ============

    // GET /api/settings
    if (method === 'GET' && pathname === '/api/settings') {
      const app = await getApp();
      const settings = app.configManager.getSystemSettings();
      const windowConfig = app.configManager.getWindowConfig();
      return sendJSON(res, 200, { success: true, data: { systemSettings: settings, windowConfig } });
    }

    // POST /api/settings
    if (method === 'POST' && pathname === '/api/settings') {
      const body = await readBody<{ systemSettings?: SystemSettings; windowConfig?: WindowConfig }>(req);
      const app = await getApp();
      if (body.systemSettings) app.configManager.saveSystemSettings(body.systemSettings);
      if (body.windowConfig) app.configManager.saveWindowConfig(body.windowConfig);
      return sendJSON(res, 200, { success: true, message: '设置已保存' });
    }

    // ============ 配置导入/导出 ============

    // GET /api/export
    if (method === 'GET' && pathname === '/api/export') {
      const app = await getApp();
      const config = app.configManager.exportAll();
      return sendJSON(res, 200, { success: true, data: config });
    }

    // POST /api/import
    if (method === 'POST' && pathname === '/api/import') {
      const body = await readBody<PersistedConfig>(req);
      const app = await getApp();
      app.configManager.importAll(body);
      app.eventManager.loadEvents(body.events);
      return sendJSON(res, 200, { success: true, message: '导入成功' });
    }

    // 404
    sendJSON(res, 404, { success: false, error: 'Route not found: ' + method + ' ' + pathname });
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    sendJSON(res, 500, { success: false, error: msg });
  }
}

// ==================== 启动服务器 ====================

export async function startApiServer(): Promise<void> {
  const server = createServer(handleRequest);

  // 挂载 Socket.IO 管理通道（用于向前端实时推送数据）
  const io = new SocketIOServer(server, {
    path: '/admin/socket.io',
    cors: { origin: '*' },
  });

  // 等待 app 初始化完成
  const app = await getApp();
  console.log('[AdminSocket] app 已初始化，准备注册 Socket.IO 管理通道');

  io.on('connection', (socket) => {
    console.log('[AdminSocket] 管理界面已连接:', socket.id);

    // 发送初始数据（runtimes + clients + logs）
    try {
      socket.emit('runtime_update', app.serviceManager.getRuntimes());
      socket.emit('client_update', app.clientManager.getClients());
      socket.emit('log_batch', app.logManager.getEntries().slice(-100)); // 只推送最近 100 条
    } catch (err) {
      console.error('[AdminSocket] 发送初始数据失败:', err);
    }

    socket.on('disconnect', () => {
      console.log('[AdminSocket] 管理界面已断开:', socket.id);
    });

    // 前端心跳响应
    socket.on('admin_ping', () => {
      socket.emit('admin_pong', Date.now());
    });
  });

  // 监听 ServiceManager 的 runtime_updated 事件，实时推送给前端
  app.serviceManager.on('runtime_updated', (runtimes: Record<string, ServerRuntime>) => {
    io.emit('runtime_update', runtimes);
  });

  // 监听 LogManager 的 log 事件，实时推送单条日志
  app.logManager.on('log', (entry: LogEntry) => {
    io.emit('log_update', entry);
  });

  // 监听客户端连接/断开事件，实时推送客户端列表
  app.serviceManager.on('client_connect', () => {
    io.emit('client_update', app.clientManager.getClients());
  });
  app.serviceManager.on('client_disconnect', () => {
    io.emit('client_update', app.clientManager.getClients());
  });

  console.log('[AdminSocket] 已注册所有实时推送事件 (runtime_update, log_update, client_update)');

  server.on('error', (err: NodeJS.ErrnoException) => {
    if (err.code === 'EADDRINUSE') {
      console.error(`[REST API] 端口 ${PORT} 已被占用，无法启动 API 服务`);
      console.error(`[REST API] 请执行：netstat -ano | findstr :${PORT}  查看占用进程`);
    } else {
      console.error('[REST API] 启动失败：', err.message);
    }
  });

  server.listen(PORT, () => {
    console.log(`[REST API] 监听 http://localhost:${PORT}`);
  });
}

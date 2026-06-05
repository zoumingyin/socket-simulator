/**
 * mcp/index.ts - MCP 工具暴露层
 * 提供：start_server / stop_server / restart_server / send_message / broadcast_message / get_clients / get_logs
 */
import { getApp } from '../main.js';
import type {
  ServerConfig,
  ClientInfo,
  LogEntry,
} from '../src/types/index';

/** MCP 工具列表（描述给 LLM 的 schema） */
export const mcpToolSchemas = [
  {
    name: 'start_server',
    description: '启动指定 ID 的 Socket 服务',
    inputSchema: {
      type: 'object',
      properties: { serverId: { type: 'string', description: '服务 ID' } },
      required: ['serverId'],
    },
  },
  {
    name: 'stop_server',
    description: '停止指定 ID 的 Socket 服务',
    inputSchema: {
      type: 'object',
      properties: { serverId: { type: 'string', description: '服务 ID' } },
      required: ['serverId'],
    },
  },
  {
    name: 'restart_server',
    description: '重启指定 ID 的 Socket 服务',
    inputSchema: {
      type: 'object',
      properties: { serverId: { type: 'string', description: '服务 ID' } },
      required: ['serverId'],
    },
  },
  {
    name: 'send_message',
    description: '向指定客户端发送事件消息',
    inputSchema: {
      type: 'object',
      properties: {
        serverId: { type: 'string', description: '服务 ID' },
        clientId: { type: 'string', description: '客户端 ID' },
        event: { type: 'string', description: '事件名称' },
        data: { type: 'object', description: '消息数据（JSON 对象）' },
      },
      required: ['serverId', 'clientId', 'event', 'data'],
    },
  },
  {
    name: 'broadcast_message',
    description: '向所有或指定客户端广播事件消息',
    inputSchema: {
      type: 'object',
      properties: {
        serverId: { type: 'string', description: '服务 ID' },
        event: { type: 'string', description: '事件名称' },
        data: { type: 'object', description: '消息数据（JSON 对象）' },
        targetIds: { type: 'array', items: { type: 'string' }, description: '可选，指定客户端 ID 列表' },
      },
      required: ['serverId', 'event', 'data'],
    },
  },
  {
    name: 'get_clients',
    description: '获取所有在线客户端列表，可按服务 ID 过滤',
    inputSchema: {
      type: 'object',
      properties: {
        serverId: { type: 'string', description: '可选，按服务 ID 过滤' },
      },
    },
  },
  {
    name: 'get_logs',
    description: '获取日志记录，支持按服务/等级/关键字过滤',
    inputSchema: {
      type: 'object',
      properties: {
        serverId: { type: 'string', description: '可选，按服务 ID 过滤' },
        level: { type: 'string', enum: ['DEBUG', 'INFO', 'WARN', 'ERROR'], description: '可选，日志等级' },
        keyword: { type: 'string', description: '可选，关键字搜索' },
      },
    },
  },
];

/** 执行 MCP 工具 */
export async function executeMcpTool(
  name: string,
  args: Record<string, unknown>
): Promise<{ content: Array<{ type: string; text: string }>; isError?: boolean }> {
  try {
    const app = await getApp();

    switch (name) {
      case 'start_server': {
        await app.serviceManager.startServer(args.serverId as string);
        return { content: [{ type: 'text', text: '服务已启动' }] };
      }
      case 'stop_server': {
        await app.serviceManager.stopServer(args.serverId as string);
        return { content: [{ type: 'text', text: '服务已停止' }] };
      }
      case 'restart_server': {
        await app.serviceManager.restartServer(args.serverId as string);
        return { content: [{ type: 'text', text: '服务已重启' }] };
      }
      case 'send_message': {
        await app.clientManager.sendToClient(
          args.serverId as string,
          args.clientId as string,
          args.event as string,
          args.data
        );
        return { content: [{ type: 'text', text: '消息已发送' }] };
      }
      case 'broadcast_message': {
        await app.clientManager.broadcast(
          args.serverId as string,
          args.event as string,
          args.data,
          args.targetIds as string[] | undefined
        );
        return { content: [{ type: 'text', text: '广播已发送' }] };
      }
      case 'get_clients': {
        const clients: ClientInfo[] = app.clientManager.getClients(
          (args.serverId as string) ?? undefined
        );
        return { content: [{ type: 'text', text: JSON.stringify(clients, null, 2) }] };
      }
      case 'get_logs': {
        const logs: LogEntry[] = app.logManager.getEntries(
          args.serverId || args.level || args.keyword
            ? {
                serverId: (args.serverId as string) || undefined,
                level: (args.level as 'DEBUG' | 'INFO' | 'WARN' | 'ERROR') || undefined,
                keyword: (args.keyword as string) || undefined,
              }
            : undefined
        );
        return { content: [{ type: 'text', text: JSON.stringify(logs.slice(-100), null, 2) }] };
      }
      default:
        return { content: [{ type: 'text', text: `未知工具: ${name}` }], isError: true };
    }
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    return { content: [{ type: 'text', text: `执行失败: ${msg}` }], isError: true };
  }
}

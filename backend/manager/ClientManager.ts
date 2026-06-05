/**
 * ClientManager - 客户端连接管理
 * 维护所有客户端的完整信息，支持分组、搜索、断开、发消息
 */
import { EventEmitter } from 'events';
import { nanoid } from 'nanoid';
import type {
  ClientInfo,
  ClientGroup,
  ClientGroupType,
  SendMessageRequest,
  ProtocolType,
} from '../src/types/index';
import type { ITransport } from '../src/types/index';

export class ClientManager extends EventEmitter {
  private clients = new Map<string, ClientInfo>();
  private groups = new Map<string, ClientGroup>();
  private transportMap = new Map<string, ITransport>();

  /** 注册传输层引用（ServiceManager 启动时调用） */
  registerTransport(serverId: string, transport: ITransport): void {
    this.transportMap.set(serverId, transport);
  }

  unregisterTransport(serverId: string): void {
    this.transportMap.delete(serverId);
  }

  /** 记录客户端连接 */
  addClient(client: ClientInfo): void {
    this.clients.set(client.id, client);
  }

  /** 记录客户端断开 */
  removeClient(clientId: string): void {
    this.clients.delete(clientId);
  }

  getClients(serverId?: string): ClientInfo[] {
    const all = Array.from(this.clients.values());
    return serverId ? all.filter((c) => c.serverId === serverId) : all;
  }

  getClient(id: string): ClientInfo | undefined {
    return this.clients.get(id);
  }

  /** 向指定客户端发送消息 */
  async sendToClient(serverId: string, clientId: string, event: string, data: unknown): Promise<void> {
    const transport = this.transportMap.get(serverId);
    if (!transport) throw new Error(`Transport not found for server ${serverId}`);
    await transport.send(clientId, event, data);
  }

  /** 广播消息 */
  async broadcast(serverId: string, event: string, data: unknown, targetIds?: string[]): Promise<void> {
    const transport = this.transportMap.get(serverId);
    if (!transport) throw new Error(`Transport not found for server ${serverId}`);
    await transport.broadcast(event, data, targetIds);
  }

  /** 断开指定客户端 */
  async disconnectClient(serverId: string, clientId: string): Promise<void> {
    const transport = this.transportMap.get(serverId);
    if (transport) {
      await transport.disconnectClient(clientId);
    }
    this.removeClient(clientId);
  }

  /** 搜索客户端 */
  search(keyword: string): ClientInfo[] {
    const lower = keyword.toLowerCase();
    return this.getClients().filter(
      (c) =>
        c.id.toLowerCase().includes(lower) ||
        c.socketId.toLowerCase().includes(lower) ||
        c.ipAddress.toLowerCase().includes(lower) ||
        c.groupName?.toLowerCase().includes(lower)
    );
  }

  // ==================== 分组管理 ====================

  createGroup(name: string, type: ClientGroupType, clientIds: string[] = []): ClientGroup {
    const group: ClientGroup = {
      id: nanoid(12),
      name,
      type,
      clientIds,
      createdAt: new Date().toISOString(),
    };
    this.groups.set(group.id, group);
    return group;
  }

  getGroups(): ClientGroup[] {
    return Array.from(this.groups.values());
  }

  addClientToGroup(groupId: string, clientId: string): boolean {
    const group = this.groups.get(groupId);
    if (!group || group.clientIds.includes(clientId)) return false;
    group.clientIds.push(clientId);
    return true;
  }

  removeClientFromGroup(groupId: string, clientId: string): boolean {
    const group = this.groups.get(groupId);
    if (!group) return false;
    group.clientIds = group.clientIds.filter((id) => id !== clientId);
    return true;
  }

  getClientsByGroup(groupId: string): ClientInfo[] {
    const group = this.groups.get(groupId);
    if (!group) return [];
    return group.clientIds
      .map((id) => this.clients.get(id))
      .filter((c): c is ClientInfo => !!c);
  }
}

import type { ProtocolType } from '../../types/index.js';

export function protocolLabel(protocol: ProtocolType | string): string {
  if (protocol === 'websocket') return 'WS';
  if (protocol === 'http') return 'HTTP';
  if (protocol === 'socket.io') return 'S.IO';
  return String(protocol).toUpperCase();
}

/** Ant Design Tag color tokens */
export function protocolTagColor(protocol: ProtocolType | string): string {
  if (protocol === 'websocket') return 'blue';
  if (protocol === 'socket.io') return 'green';
  if (protocol === 'http') return 'cyan';
  return 'default';
}

/** HTTP 服务才展示 Mock / 试跑 */
export function isHttpService(protocol: ProtocolType | string): boolean {
  return protocol === 'http';
}

export type WorkbenchSection = 'overview' | 'basics' | 'http-mock';

export function isHttpOnlySection(section: WorkbenchSection): boolean {
  return section === 'http-mock';
}

/**
 * v3 P1-2 服务字段注册表（field registry）
 *
 * 服务表单（BasicsSection）由注册表驱动渲染，避免每协议一套表单：
 * - 每字段声明 名称 / 标签 / 控件类型 / 校验 / 布局 / 按协议可见性
 * - 新增字段只需在注册表加一条，表单层零改动
 * - `visible` 按 `protocol` 决定字段是否展示（如 WSS 仅 WebSocket 可用）
 */
import type { ProtocolType } from '../../types/index.js';

export type FieldControl = 'text' | 'number' | 'select' | 'switch';

export interface FieldOption {
  value: string;
  label: string;
}

export interface FieldDef {
  /** 表单字段名（与 ServerConfig camelCase 对齐） */
  name: string;
  label: string;
  control: FieldControl;
  required?: boolean;
  /** AntD Col 响应式 span（xs 恒 24） */
  span?: number;
  /** 按协议可见性；缺省 = 全部协议可见 */
  visible?: (protocol: ProtocolType) => boolean;
  options?: FieldOption[];
  placeholder?: string;
  valuePropName?: 'checked';
  min?: number;
  max?: number;
}

/** 仅 WebSocket 支持的字段（如 WSS） */
const websocketOnly = (p: ProtocolType): boolean => p === 'websocket';

/**
 * 服务基础配置字段注册表（单实体覆盖三协议）。
 * 字段值经 `updateServer` 以 camelCase 直接写入 ServerConfig。
 */
export const serverFieldRegistry: FieldDef[] = [
  {
    name: 'name',
    label: '服务名称',
    control: 'text',
    required: true,
    span: 14,
    placeholder: '必填',
  },
  {
    name: 'protocol',
    label: '协议类型',
    control: 'select',
    required: true,
    span: 10,
    options: [
      { value: 'websocket', label: 'WebSocket' },
      { value: 'socket.io', label: 'Socket.IO' },
      { value: 'http', label: 'HTTP' },
    ],
  },
  {
    name: 'ip',
    label: '监听 IP',
    control: 'text',
    required: true,
    span: 14,
    placeholder: '0.0.0.0',
  },
  {
    name: 'port',
    label: '监听端口',
    control: 'number',
    required: true,
    span: 10,
    min: 1,
    max: 65535,
  },
  {
    name: 'autoStart',
    label: '自动启动',
    control: 'switch',
    span: 10,
    valuePropName: 'checked',
  },
  {
    name: 'logLevel',
    label: '日志等级',
    control: 'select',
    span: 14,
    options: [
      { value: 'DEBUG', label: 'DEBUG' },
      { value: 'INFO', label: 'INFO' },
      { value: 'WARN', label: 'WARN' },
      { value: 'ERROR', label: 'ERROR' },
    ],
  },
  {
    name: 'wssEnabled',
    label: '启用 WSS（TLS）',
    control: 'switch',
    visible: websocketOnly,
    span: 10,
    valuePropName: 'checked',
  },
  {
    name: 'description',
    label: '描述',
    control: 'text',
    span: 24,
    placeholder: '可选',
  },
];

/** 按协议过滤出可见字段 */
export function visibleFields(protocol: ProtocolType): FieldDef[] {
  return serverFieldRegistry.filter((f) => f.visible?.(protocol) ?? true);
}

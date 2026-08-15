import { Tag, Typography } from 'antd';
const { Text } = Typography;
import type { ColumnsType } from 'antd/es/table';
import type { LogEntry, LogLevel } from '../../types/index.js';

/** 表格自适应列宽配置：固定宽度 + 长文本省略 + 消息列换行 */
export const COL_WIDTHS = {
  time: 152,
  level: 64,
  event: 110,
  serverId: 96,
  clientId: 106,
  pushEvent: 96,
  targetType: 86,
  targetId: 86,
};

export const levelColors: Record<LogLevel, string> = {
  DEBUG: 'blue',
  INFO: 'green',
  WARN: 'orange',
  ERROR: 'red',
};

export const logColumns: ColumnsType<LogEntry> = [
  {
    title: '时间',
    dataIndex: 'timestamp',
    key: 'timestamp',
    align: 'center' as const,
    width: COL_WIDTHS.time,
    render: (v: string) => (
      <Text type="secondary" style={{ fontSize: 12 }}>
        {new Date(v).toLocaleString()}
      </Text>
    ),
  },
  {
    title: '等级',
    dataIndex: 'level',
    key: 'level',
    align: 'center' as const,
    width: COL_WIDTHS.level,
    render: (v: LogLevel) => <Tag color={levelColors[v]}>{v}</Tag>,
  },
  {
    title: '事件',
    dataIndex: 'event',
    key: 'event',
    align: 'center' as const,
    width: COL_WIDTHS.event,
    ellipsis: true,
  },
  {
    title: '服务ID',
    dataIndex: 'serverId',
    key: 'serverId',
    align: 'center' as const,
    width: COL_WIDTHS.serverId,
    ellipsis: true,
    render: (v?: string) =>
      v ? (
        <Tag
          style={{
            maxWidth: '100%',
            overflow: 'hidden',
            textOverflow: 'ellipsis',
          }}
        >
          {v}
        </Tag>
      ) : (
        '-'
      ),
  },
  {
    title: '客户端ID',
    dataIndex: 'clientId',
    key: 'clientId',
    align: 'center' as const,
    width: COL_WIDTHS.clientId,
    ellipsis: true,
    render: (v?: string) =>
      v ? (
        <Text code style={{ fontSize: 12 }}>
          {v}
        </Text>
      ) : (
        '-'
      ),
  },
  {
    title: '推送事件',
    key: 'pushEvent',
    align: 'center' as const,
    width: COL_WIDTHS.pushEvent,
    ellipsis: true,
    render: (_: unknown, record: LogEntry) =>
      record.metadata?.event ? (
        <Tag color="blue">{String(record.metadata.event)}</Tag>
      ) : (
        '-'
      ),
  },
  {
    title: '目标类型',
    key: 'targetType',
    align: 'center' as const,
    width: COL_WIDTHS.targetType,
    ellipsis: true,
    render: (_: unknown, record: LogEntry) =>
      record.metadata?.targetType ? (
        <Tag color="green">{String(record.metadata.targetType)}</Tag>
      ) : (
        '-'
      ),
  },
  {
    title: '目标ID',
    key: 'targetId',
    align: 'center' as const,
    width: COL_WIDTHS.targetId,
    ellipsis: true,
    render: (_: unknown, record: LogEntry) =>
      record.metadata?.targetId ? (
        <Text code style={{ fontSize: 12 }}>
          {String(record.metadata.targetId)}
        </Text>
      ) : (
        '-'
      ),
  },
  {
    title: '消息',
    dataIndex: 'message',
    key: 'message',
    align: 'left' as const,
    render: (text: string) => (
      <div
        style={{
          maxHeight: 48,
          overflow: 'hidden',
          wordBreak: 'break-all',
          whiteSpace: 'pre-wrap',
          lineHeight: '20px',
          fontSize: 12,
          cursor: 'pointer',
        }}
      >
        {text}
      </div>
    ),
  },
];

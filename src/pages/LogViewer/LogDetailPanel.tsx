import { Card, Space, Button, Tag, Typography, theme } from 'antd';
const { Text } = Typography;
import { CloseOutlined } from '@ant-design/icons';
import type { LogEntry } from '../../types/index.js';
import { levelColors } from './logColumns.js';

/** 格式化 metadata，单独解析 content 字段避免转义符 */
export function formatMetadata(
  metadata: Record<string, unknown> | undefined
): string {
  if (!metadata) return '{}';
  try {
    const meta = JSON.parse(JSON.stringify(metadata));
    if (meta.content && typeof meta.content === 'string') {
      try {
        meta.content = JSON.parse(meta.content);
      } catch {}
    }
    return JSON.stringify(meta, null, 2);
  } catch {
    return JSON.stringify(metadata, null, 2);
  }
}

export function LogDetailPanel({
  selectedLog,
  onClose,
}: {
  selectedLog: LogEntry | null;
  onClose: () => void;
}) {
  if (!selectedLog) return null;
  const record = selectedLog;
  const hasMetadata =
    record.metadata && Object.keys(record.metadata).length > 0;
  const { token } = theme.useToken();

  return (
    <Card
      variant="outlined"
      style={{ marginTop: 12, backgroundColor: token.colorFillAlter }}
      title={
        <Space>
          <Tag color={levelColors[record.level ?? 'INFO']}>{record.level ?? 'INFO'}</Tag>
          <Text strong>{record.event}</Text>
        </Space>
      }
      extra={
        <Button
          type="text"
          size="small"
          icon={<CloseOutlined />}
          onClick={onClose}
        />
      }
    >
      <Space direction="vertical" style={{ width: '100%' }} size={16}>
        {/* 基本信息 */}
        <div>
          <Text
            type="secondary"
            style={{ display: 'block', marginBottom: 8 }}
          >
            基本信息
          </Text>
          <Space wrap size={[16, 8]}>
            <div>
              <Text type="secondary">时间：</Text>
              <Text style={{ fontSize: 13 }}>
                {new Date(record.timestamp ?? '').toLocaleString()}
              </Text>
            </div>
            <div>
              <Text type="secondary">服务ID：</Text>
              {record.serverId ? (
                <Tag>{record.serverId}</Tag>
              ) : (
                <Text type="secondary">-</Text>
              )}
            </div>
            <div>
              <Text type="secondary">客户端ID：</Text>
              {record.clientId ? (
                <Text code>{record.clientId}</Text>
              ) : (
                <Text type="secondary">-</Text>
              )}
            </div>
          </Space>
        </div>

        {/* 推送信息 */}
        {hasMetadata && (
          <div>
            <Text
              type="secondary"
              style={{ display: 'block', marginBottom: 8 }}
            >
              推送信息
            </Text>
            <Space wrap size={[16, 8]}>
              {record.metadata?.event ? (
                <div>
                  <Text type="secondary">推送事件：</Text>
                  <Tag color="blue">{String(record.metadata.event)}</Tag>
                </div>
              ) : null}
              {record.metadata?.targetType ? (
                <div>
                  <Text type="secondary">目标类型：</Text>
                  <Tag color="green">
                    {String(record.metadata.targetType)}
                  </Tag>
                </div>
              ) : null}
              {record.metadata?.targetId ? (
                <div>
                  <Text type="secondary">目标ID：</Text>
                  <Text code style={{ fontSize: 12 }}>
                    {String(record.metadata.targetId)}
                  </Text>
                </div>
              ) : null}
            </Space>
          </div>
        )}

        {/* 消息内容 */}
        <div>
          <Text
            type="secondary"
            style={{ display: 'block', marginBottom: 8 }}
          >
            消息内容
          </Text>
          <pre
            style={{
              backgroundColor: token.colorBgContainer,
              border: `1px solid ${token.colorBorder}`,
              borderRadius: 6,
              padding: 12,
              maxHeight: 400,
              overflow: 'auto',
              whiteSpace: 'pre-wrap',
              wordBreak: 'break-all',
              fontFamily:
                "'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, monospace",
              fontSize: 13,
              lineHeight: 1.6,
              margin: 0,
            }}
          >
            {record.message || <Text type="secondary">（无消息内容）</Text>}
          </pre>
        </div>

        {/* 原始 metadata —— 单独解析 content 字段避免转义符 */}
        {hasMetadata && (
          <div>
            <Text
              type="secondary"
              style={{ display: 'block', marginBottom: 8 }}
            >
              原始 Metadata
            </Text>
            <div
              style={{
                backgroundColor: token.colorBgContainer,
                border: `1px solid ${token.colorBorder}`,
                borderRadius: 6,
                padding: 12,
                maxHeight: 300,
                overflow: 'auto',
                whiteSpace: 'pre-wrap',
                wordBreak: 'break-all',
                fontFamily:
                  "'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, monospace",
                fontSize: 12,
                lineHeight: 1.5,
                color: '#595959',
              }}
            >
              {formatMetadata(record.metadata)}
            </div>
          </div>
        )}
      </Space>
    </Card>
  );
}

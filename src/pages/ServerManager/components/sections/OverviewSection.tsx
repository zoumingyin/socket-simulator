import React from 'react';
import { Button, Space, Tag, Typography, theme } from 'antd';
import type { ServerConfig, ServerRuntime } from '../../../../types/index.js';
import { isHttpService, protocolLabel, protocolTagColor } from '../../protocolStyles.js';

const { Text, Title } = Typography;

function StatBlock({ label, value, accent }: { label: string; value: React.ReactNode; accent?: string }) {
  const { token } = theme.useToken();
  return (
    <div
      style={{
        padding: '12px 14px',
        borderRadius: 8,
        border: `1px solid ${token.colorBorderSecondary}`,
        background: token.colorFillAlter,
        minWidth: 0,
      }}
    >
      <Text type="secondary" style={{ fontSize: 12, display: 'block', marginBottom: 4 }}>{label}</Text>
      <Text strong style={{ fontSize: 20, color: accent, lineHeight: 1.2 }}>{value}</Text>
    </div>
  );
}

export function OverviewSection({
  server,
  runtime,
  onGoHttpMock,
}: {
  server: ServerConfig;
  runtime?: ServerRuntime;
  onGoHttpMock: () => void;
}): React.ReactElement {
  const { token } = theme.useToken();
  const running = runtime?.status === 'running';
  const http = isHttpService(server.protocol);
  const ruleCount = server.mockRules?.length ?? 0;
  const enabledRules = (server.mockRules ?? []).filter((r: { enabled: boolean }) => r.enabled).length;

  return (
    <Space direction="vertical" size={16} style={{ width: '100%' }}>
      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fill, minmax(140px, 1fr))',
          gap: 10,
        }}
      >
        <StatBlock
          label="在线客户端"
          value={runtime?.clientCount ?? 0}
          accent={running && (runtime?.clientCount ?? 0) > 0 ? '#52c41a' : undefined}
        />
        <StatBlock label="总连接" value={runtime?.totalConnections ?? 0} />
        <StatBlock label="发送消息" value={runtime?.sentMessages ?? 0} />
        <StatBlock label="接收消息" value={runtime?.receivedMessages ?? 0} />
      </div>

      <div
        style={{
          padding: 14,
          borderRadius: 8,
          border: `1px solid ${token.colorBorderSecondary}`,
        }}
      >
        <Title level={5} style={{ marginTop: 0, marginBottom: 10, fontSize: 14 }}>服务摘要</Title>
        <Space direction="vertical" size={6} style={{ width: '100%' }}>
          <div>
            <Text type="secondary">协议 </Text>
            <Tag color={protocolTagColor(server.protocol)}>{protocolLabel(server.protocol)}</Tag>
            <Text type="secondary">地址 </Text>
            <Text code>{server.ip}:{server.port}</Text>
          </div>
          {http && (
            <div>
              <Text type="secondary">Mock </Text>
              {server.mockEnabled ? (
                <Text>
                  已开启 · {enabledRules}/{ruleCount} 条规则启用 · 默认 {server.mockDefaultStatusCode ?? 200}
                </Text>
              ) : (
                <Text type="secondary">未开启</Text>
              )}
            </div>
          )}
          {server.description && (
            <div>
              <Text type="secondary">描述 </Text>
              <Text>{server.description}</Text>
            </div>
          )}
        </Space>
        {http && (
          <Space style={{ marginTop: 12 }}>
            <Button size="small" type="primary" ghost onClick={onGoHttpMock}>配置规则 / 试跑</Button>
          </Space>
        )}
      </div>
    </Space>
  );
}

import React, { useEffect } from 'react';
import { Button, Segmented, Space, Tag, Typography, theme } from 'antd';
import type { MessageInstance } from 'antd/es/message/interface';
import type { ServerConfig, ServerRuntime, MockRule } from '../../../types/index.js';
import {
  isHttpOnlySection,
  isHttpService,
  protocolLabel,
  protocolTagColor,
  type WorkbenchSection,
} from '../protocolStyles.js';
import { OverviewSection } from './sections/OverviewSection.js';
import { BasicsSection } from './sections/BasicsSection.js';
import { HttpMockSection } from './sections/HttpMockSection.js';
import { ProbeSection } from './sections/ProbeSection.js';

const { Title, Text } = Typography;

export function ServerWorkbench({
  server,
  runtime,
  section,
  onSectionChange,
  onSaveBasics,
  onSaveMock,
  onUpdateMockRules,
  onStart,
  onStop,
  onRestart,
  messageApi,
}: {
  server: ServerConfig;
  runtime?: ServerRuntime;
  section: WorkbenchSection;
  onSectionChange: (s: WorkbenchSection) => void;
  onSaveBasics: (vals: Record<string, unknown>) => Promise<void>;
  onSaveMock: (vals: Record<string, unknown>) => Promise<void>;
  onUpdateMockRules: (rules: MockRule[]) => Promise<void>;
  onStart: () => void;
  onStop: () => void;
  onRestart: () => void;
  messageApi: MessageInstance;
}): React.ReactElement {
  const { token } = theme.useToken();
  const running = runtime?.status === 'running';
  const http = isHttpService(server.protocol);

  useEffect(() => {
    if (!http && isHttpOnlySection(section)) {
      onSectionChange('overview');
    }
  }, [http, section, onSectionChange]);

  const sectionOptions = [
    { value: 'overview', label: '概览' },
    { value: 'basics', label: '基础' },
    ...(http
      ? [
          { value: 'http-mock', label: 'HTTP·Mock' },
          { value: 'probe', label: '试跑' },
        ]
      : []),
  ];

  const activeSection = !http && isHttpOnlySection(section) ? 'overview' : section;

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', minHeight: 0 }}>
      <div
        style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'flex-start',
          gap: 16,
          marginBottom: 16,
        }}
      >
        <div style={{ minWidth: 0 }}>
          <Title level={4} style={{ margin: '0 0 6px', fontSize: 18, fontWeight: 600 }}>
            {server.name || server.id}
          </Title>
          <Space size={6} wrap>
            <Tag color={protocolTagColor(server.protocol)} style={{ margin: 0 }}>
              {protocolLabel(server.protocol)}
            </Tag>
            {http && server.mockEnabled && (
              <Tag color="orange" style={{ margin: 0 }}>Mock</Tag>
            )}
            <Tag color={running ? 'success' : 'default'} style={{ margin: 0 }}>
              {running ? '运行中' : '已停止'}
            </Tag>
            <Text type="secondary" style={{ fontSize: 12, fontFamily: token.fontFamilyCode }}>
              {server.ip}:{server.port}
            </Text>
          </Space>
        </div>
        <Space>
          {!running && (
            <Button type="primary" onClick={onStart}>启动</Button>
          )}
          {running && (
            <Button danger onClick={onStop}>停止</Button>
          )}
          {running && (
            <Button onClick={onRestart}>重启</Button>
          )}
        </Space>
      </div>

      <Segmented
        value={activeSection}
        onChange={(v) => onSectionChange(v as WorkbenchSection)}
        options={sectionOptions}
        style={{ marginBottom: 16, alignSelf: 'flex-start' }}
      />

      <div style={{ flex: 1, overflow: 'auto', minHeight: 0, paddingRight: 4 }}>
        {activeSection === 'overview' && (
          <OverviewSection
            server={server}
            runtime={runtime}
            onGoHttpMock={() => onSectionChange('http-mock')}
            onGoProbe={() => onSectionChange('probe')}
          />
        )}
        {activeSection === 'basics' && (
          <BasicsSection server={server} onSave={onSaveBasics} />
        )}
        {http && activeSection === 'http-mock' && (
          <HttpMockSection
            server={server}
            onSave={onSaveMock}
            onUpdateRules={onUpdateMockRules}
            messageApi={messageApi}
          />
        )}
        {http && activeSection === 'probe' && (
          <ProbeSection server={server} messageApi={messageApi} />
        )}
      </div>
    </div>
  );
}

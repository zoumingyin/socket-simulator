import React, { useEffect } from 'react';
import {
  Button, Form, InputNumber, Switch, Typography, theme,
} from 'antd';
import type { MessageInstance } from 'antd/es/message/interface';
import type { MockRule, ServerConfig } from '../../../../types/index.js';
import { JsonEditor, MockRulesTable } from '../../../../components/MockComponents.js';

const { Text, Title } = Typography;

export function HttpMockSection({
  server,
  onSave,
  onUpdateRules,
  messageApi,
}: {
  server: ServerConfig;
  onSave: (vals: Record<string, unknown>) => Promise<void>;
  onUpdateRules: (rules: MockRule[]) => Promise<void>;
  messageApi: MessageInstance;
}): React.ReactElement {
  const { token } = theme.useToken();
  const [form] = Form.useForm();
  const mockEnabled = Form.useWatch('mockEnabled', form);
  const rules = server.mockRules ?? [];
  const enabledCount = rules.filter((r) => r.enabled).length;

  useEffect(() => {
    form.setFieldsValue({
      mockEnabled: server.mockEnabled ?? false,
      mockDefaultStatusCode: server.mockDefaultStatusCode ?? 200,
      mockDefaultResponseBody: server.mockDefaultResponseBody ?? '{"message":"ok"}',
      mockDefaultDelayMs: server.mockDefaultDelayMs ?? 0,
    });
  }, [server, form]);

  const panelStyle: React.CSSProperties = {
    border: `1px solid ${token.colorBorderSecondary}`,
    borderRadius: 8,
    background: token.colorFillAlter,
    padding: 12,
    minWidth: 0,
  };

  return (
    <div
      style={{
        width: '100%',
        maxWidth: '100%',
        minWidth: 0,
        overflowX: 'hidden',
        display: 'flex',
        flexDirection: 'column',
        gap: 12,
      }}
    >
      <Form
        form={form}
        layout="vertical"
        size="small"
        style={{ maxWidth: '100%' }}
        onFinish={(v) => onSave({
          mockEnabled: v.mockEnabled,
          mockDefaultStatusCode: v.mockDefaultStatusCode,
          mockDefaultResponseBody: v.mockDefaultResponseBody,
          mockDefaultDelayMs: v.mockDefaultDelayMs,
        })}
      >
        {/* 顶栏：开关 + 说明 + 保存 */}
        <div
          style={{
            ...panelStyle,
            padding: '14px 16px',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            gap: 16,
            flexWrap: 'wrap',
            marginBottom: 12,
          }}
        >
          <div style={{ display: 'flex', alignItems: 'center', gap: 16, minWidth: 0, flex: 1 }}>
            <div style={{ display: 'flex', flexDirection: 'column', gap: 6, flexShrink: 0 }}>
              <Text style={{ fontSize: 12, color: token.colorTextSecondary, lineHeight: 1 }}>Mock HTTP</Text>
              <Form.Item name="mockEnabled" valuePropName="checked" style={{ marginBottom: 0 }}>
                <Switch checkedChildren="开" unCheckedChildren="关" />
              </Form.Item>
            </div>
            <div
              style={{
                width: 1,
                alignSelf: 'stretch',
                background: token.colorBorderSecondary,
                flexShrink: 0,
              }}
            />
            <Text type="secondary" style={{ fontSize: 12, lineHeight: 1.5, minWidth: 0 }}>
              {mockEnabled
                ? `已启用 · ${enabledCount}/${rules.length} 条规则生效；未命中规则时走下方默认响应`
                : '关闭时仅提供 HTTP 传输默认路由，不走 Mock 规则'}
            </Text>
          </div>
          <Button type="primary" htmlType="submit">
            保存设置
          </Button>
        </div>

        {mockEnabled && (
          <div
            style={{
              display: 'grid',
              gridTemplateColumns: 'minmax(0, 280px) minmax(0, 1fr)',
              gap: 12,
              marginBottom: 12,
              alignItems: 'stretch',
            }}
            className="http-mock-defaults-grid"
          >
            <div style={panelStyle}>
              <Title level={5} style={{ margin: '0 0 10px', fontSize: 13, fontWeight: 600 }}>
                默认响应
              </Title>
              <Text type="secondary" style={{ fontSize: 12, display: 'block', marginBottom: 10 }}>
                未命中任何规则时返回
              </Text>
              <Form.Item
                name="mockDefaultStatusCode"
                label="状态码"
                rules={[{ required: true }]}
                style={{ marginBottom: 8 }}
              >
                <InputNumber min={100} max={599} style={{ width: '100%' }} />
              </Form.Item>
              <Form.Item name="mockDefaultDelayMs" label="延迟 (ms)" style={{ marginBottom: 0 }}>
                <InputNumber min={0} max={60000} style={{ width: '100%' }} />
              </Form.Item>
            </div>

            <div style={{ ...panelStyle, display: 'flex', flexDirection: 'column' }}>
              <Title level={5} style={{ margin: '0 0 10px', fontSize: 13, fontWeight: 600 }}>
                默认响应体
              </Title>
              <Form.Item
                name="mockDefaultResponseBody"
                rules={[{ required: true }]}
                style={{ marginBottom: 0, flex: 1 }}
              >
                <JsonEditor rows={5} />
              </Form.Item>
            </div>
          </div>
        )}
      </Form>

      {mockEnabled ? (
        <div style={{ ...panelStyle, padding: '12px 12px 8px' }}>
          <div
            style={{
              display: 'flex',
              alignItems: 'baseline',
              justifyContent: 'space-between',
              gap: 8,
              marginBottom: 4,
            }}
          >
            <Title level={5} style={{ margin: 0, fontSize: 13, fontWeight: 600 }}>
              匹配规则
            </Title>
            <Text type="secondary" style={{ fontSize: 12 }}>
              按顺序匹配，首个命中生效
            </Text>
          </div>
          <MockRulesTable
            rules={rules}
            onUpdate={onUpdateRules}
            messageApi={messageApi}
          />
        </div>
      ) : (
        <div style={{ ...panelStyle, textAlign: 'center', padding: '28px 16px' }}>
          <Text type="secondary">开启 Mock 后可配置默认响应与匹配规则，并在「试跑」中验证。</Text>
        </div>
      )}

      <style>{`
        @media (max-width: 900px) {
          .http-mock-defaults-grid {
            grid-template-columns: 1fr !important;
          }
        }
      `}</style>
    </div>
  );
}

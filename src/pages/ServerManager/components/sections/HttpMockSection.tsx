import React, { useEffect } from 'react';
import {
  Button, Col, Form, InputNumber, Row, Space, Switch, Typography,
} from 'antd';
import type { MessageInstance } from 'antd/es/message/interface';
import type { MockRule, ServerConfig } from '../../../../types/index.js';
import { JsonEditor, MockRulesTable } from '../../../../components/MockComponents.js';

const { Text } = Typography;

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
  const [form] = Form.useForm();
  const mockEnabled = Form.useWatch('mockEnabled', form);

  useEffect(() => {
    form.setFieldsValue({
      mockEnabled: server.mockEnabled ?? false,
      mockDefaultStatusCode: server.mockDefaultStatusCode ?? 200,
      mockDefaultResponseBody: server.mockDefaultResponseBody ?? '{"message":"ok"}',
      mockDefaultDelayMs: server.mockDefaultDelayMs ?? 0,
    });
  }, [server, form]);

  return (
    <Space direction="vertical" size={16} style={{ width: '100%' }}>
      <Form
        form={form}
        layout="vertical"
        onFinish={(v) => onSave({
          mockEnabled: v.mockEnabled,
          mockDefaultStatusCode: v.mockDefaultStatusCode,
          mockDefaultResponseBody: v.mockDefaultResponseBody,
          mockDefaultDelayMs: v.mockDefaultDelayMs,
        })}
      >
        <Form.Item
          name="mockEnabled"
          label="启用 Mock HTTP"
          valuePropName="checked"
          extra="未匹配规则时返回下方默认响应；匹配规则按顺序首个命中生效"
        >
          <Switch checkedChildren="开" unCheckedChildren="关" />
        </Form.Item>

        {mockEnabled && (
          <>
            <Row gutter={16}>
              <Col xs={12} md={8}>
                <Form.Item name="mockDefaultStatusCode" label="默认状态码" rules={[{ required: true }]}>
                  <InputNumber min={100} max={599} style={{ width: '100%' }} />
                </Form.Item>
              </Col>
              <Col xs={12} md={8}>
                <Form.Item name="mockDefaultDelayMs" label="默认延迟 (ms)">
                  <InputNumber min={0} max={60000} style={{ width: '100%' }} />
                </Form.Item>
              </Col>
            </Row>
            <Form.Item name="mockDefaultResponseBody" label="默认响应体" rules={[{ required: true }]}>
              <JsonEditor rows={4} />
            </Form.Item>
          </>
        )}

        <Form.Item>
          <Button type="primary" htmlType="submit">保存 Mock 设置</Button>
        </Form.Item>
      </Form>

      {mockEnabled ? (
        <div>
          <Text strong style={{ display: 'block', marginBottom: 8 }}>匹配规则</Text>
          <MockRulesTable
            rules={server.mockRules ?? []}
            onUpdate={onUpdateRules}
            messageApi={messageApi}
          />
        </div>
      ) : (
        <Text type="secondary">开启 Mock 后可在此编辑规则，并在「试跑」中验证接口。</Text>
      )}
    </Space>
  );
}

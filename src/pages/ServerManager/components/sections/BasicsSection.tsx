import React, { useEffect } from 'react';
import { Button, Col, Form, Input, InputNumber, Row, Select, Switch } from 'antd';
import type { ServerConfig, ProtocolType } from '../../../../types/index.js';
import { visibleFields, type FieldDef } from '../../fieldRegistry.js';

const { Option } = Select;

/** 按字段定义渲染对应控件 */
function renderControl(field: FieldDef): React.ReactElement {
  switch (field.control) {
    case 'number':
      return (
        <InputNumber
          min={field.min}
          max={field.max}
          style={{ width: '100%' }}
        />
      );
    case 'select':
      return (
        <Select placeholder={field.placeholder}>
          {(field.options ?? []).map((o) => (
            <Option key={o.value} value={o.value}>
              {o.label}
            </Option>
          ))}
        </Select>
      );
    case 'switch':
      return <Switch checkedChildren="是" unCheckedChildren="否" />;
    case 'text':
    default:
      return <Input placeholder={field.placeholder} />;
  }
}

export function BasicsSection({
  server,
  onSave,
}: {
  server: ServerConfig;
  onSave: (vals: Record<string, unknown>) => Promise<void>;
}): React.ReactElement {
  const [form] = Form.useForm();

  useEffect(() => {
    form.setFieldsValue({
      name: server.name,
      description: server.description,
      protocol: server.protocol,
      ip: server.ip,
      port: server.port,
      autoStart: server.autoStart,
      logLevel: server.logLevel,
      wssEnabled: server.wssEnabled,
    });
  }, [server, form]);

  // 注册表驱动：协议切换联动字段可见性（如 WSS 仅 WebSocket）
  const protocol = (Form.useWatch('protocol', form) ?? server.protocol) as ProtocolType;
  const fields = visibleFields(protocol);

  return (
    <Form
      form={form}
      layout="vertical"
      onFinish={(v) => onSave(v)}
    >
      <Row gutter={16}>
        {fields.map((field) => (
          <Col key={field.name} xs={24} md={field.span ?? 12}>
            <Form.Item
              name={field.name}
              label={field.label}
              rules={
                field.required
                  ? [{ required: true, message: `请输入${field.label}` }]
                  : undefined
              }
              valuePropName={field.valuePropName}
            >
              {renderControl(field)}
            </Form.Item>
          </Col>
        ))}
      </Row>
      <Form.Item>
        <Button type="primary" htmlType="submit">保存</Button>
      </Form.Item>
    </Form>
  );
}

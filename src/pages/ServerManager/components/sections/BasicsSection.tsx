import React, { useEffect } from 'react';
import { Button, Col, Form, Input, InputNumber, Row, Select, Switch } from 'antd';
import type { ServerConfig } from '../../../../types/index.js';

const { Option } = Select;

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
    });
  }, [server, form]);

  return (
    <Form
      form={form}
      layout="vertical"
      onFinish={(v) => onSave(v)}
    >
      <Row gutter={16}>
        <Col xs={24} md={14}>
          <Form.Item name="name" label="服务名称" rules={[{ required: true }]}>
            <Input />
          </Form.Item>
        </Col>
        <Col xs={24} md={10}>
          <Form.Item name="protocol" label="协议类型" rules={[{ required: true }]}>
            <Select>
              <Option value="websocket">WebSocket</Option>
              <Option value="socket.io">Socket.IO</Option>
              <Option value="http">HTTP</Option>
            </Select>
          </Form.Item>
        </Col>
        <Col xs={24} md={14}>
          <Form.Item name="ip" label="监听 IP" rules={[{ required: true }]}>
            <Input placeholder="0.0.0.0" />
          </Form.Item>
        </Col>
        <Col xs={24} md={10}>
          <Form.Item name="port" label="监听端口" rules={[{ required: true }]}>
            <InputNumber min={1} max={65535} style={{ width: '100%' }} />
          </Form.Item>
        </Col>
        <Col xs={24} md={10}>
          <Form.Item name="autoStart" label="自动启动" valuePropName="checked">
            <Switch checkedChildren="是" unCheckedChildren="否" />
          </Form.Item>
        </Col>
        <Col xs={24} md={14}>
          <Form.Item name="logLevel" label="日志等级">
            <Select>
              <Option value="DEBUG">DEBUG</Option>
              <Option value="INFO">INFO</Option>
              <Option value="WARN">WARN</Option>
              <Option value="ERROR">ERROR</Option>
            </Select>
          </Form.Item>
        </Col>
        <Col span={24}>
          <Form.Item name="description" label="描述">
            <Input placeholder="可选" />
          </Form.Item>
        </Col>
      </Row>
      <Form.Item>
        <Button type="primary" htmlType="submit">保存</Button>
      </Form.Item>
    </Form>
  );
}

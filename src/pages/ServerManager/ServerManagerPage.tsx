/**
 * ServerManagerPage - 服务管理页面
 * 支持：新增/编辑/删除/启动/停止/重启服务，以及启动全部/停止全部/重启全部
 */
import React, { useEffect, useState } from 'react';
import {
  Card, Button, Table, Tag, Space, Modal, Form, Input, InputNumber,
  Select, Popconfirm, Typography, message,
} from 'antd';
import {
  PlusOutlined, PlayCircleOutlined, PauseCircleOutlined,
  ReloadOutlined, EditOutlined, DeleteOutlined,
} from '@ant-design/icons';
import type { ServerConfig } from '../../types/index.js';
import { useServerStore } from '../../store/useServerStore.js';

const { Title } = Typography;
const { Option } = Select;

export function ServerManagerPage(): React.ReactElement {
  const [messageApi, contextHolder] = message.useMessage();
  const { list, runtimes, loading, fetchServers, fetchRuntimes, addServer, updateServer, removeServer, startServer, stopServer, restartServer, startAll, stopAll, restartAll } = useServerStore();
  const [modalOpen, setModalOpen] = useState(false);
  const [editing, setEditing] = useState<ServerConfig | null>(null);
  const [form] = Form.useForm();

  useEffect(() => { fetchServers(); fetchRuntimes(); }, []);

  // 轮询运行时状态（每 3 秒）
  useEffect(() => {
    const timer = setInterval(() => fetchRuntimes(), 3000);
    return () => clearInterval(timer);
  }, []);

  const openAdd = () => { setEditing(null); form.resetFields(); setModalOpen(true); };
  const openEdit = (r: ServerConfig) => { setEditing(r); form.setFieldsValue(r); setModalOpen(true); };

  const handleSave = async () => {
    const vals = await form.validateFields();
    try {
      if (editing) {
        await updateServer(editing.id, vals);
        messageApi.success('更新成功');
      } else {
        await addServer(vals as Omit<ServerConfig, 'id' | 'createdAt' | 'updatedAt'>);
        messageApi.success('添加成功');
      }
      setModalOpen(false);
      form.resetFields();
    } catch (e) {
      messageApi.error('保存失败：' + (e as Error).message);
    }
  };

  const columns = [
    { title: '名称', dataIndex: 'name', key: 'name' },
    { title: '协议', dataIndex: 'protocol', key: 'protocol',
      render: (v: string) => <Tag>{v.toUpperCase()}</Tag>,
    },
    { title: 'IP', dataIndex: 'ip', key: 'ip' },
    { title: '端口', dataIndex: 'port', key: 'port' },
    { title: '状态', key: 'status',
      render: (_: unknown, r: ServerConfig) => {
        const rt = runtimes[r.id];
        const running = rt?.status === 'running';
        return <Tag color={running ? 'green' : 'default'}>{running ? '运行中' : '已停止'}</Tag>;
      },
    },
    { title: '自动启动', dataIndex: 'autoStart', key: 'autoStart',
      render: (v: boolean) => v ? '是' : '否',
    },
    { title: '操作', key: 'actions', width: 280,
      render: (_: unknown, r: ServerConfig) => {
        const rt = runtimes[r.id];
        const running = rt?.status === 'running';
        return (
          <Space>
            {!running && <Button type="link" icon={<PlayCircleOutlined />} onClick={() => startServer(r.id)}>启动</Button>}
            {running && <Button type="link" danger icon={<PauseCircleOutlined />} onClick={() => stopServer(r.id)}>停止</Button>}
            {running && <Button type="link" icon={<ReloadOutlined />} onClick={() => restartServer(r.id)}>重启</Button>}
            <Button type="link" icon={<EditOutlined />} onClick={() => openEdit(r)}>编辑</Button>
            <Popconfirm title="确认删除？" onConfirm={() => removeServer(r.id)}>
              <Button type="link" danger icon={<DeleteOutlined />}>删除</Button>
            </Popconfirm>
          </Space>
        );
      },
    },
  ];

  return (
    <div>
      {contextHolder}
      <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 16 }}>
        <Title level={4} style={{ margin: 0 }}>服务管理</Title>
        <Space>
          <Button type="primary" icon={<PlusOutlined />} onClick={openAdd}>新增服务</Button>
          <Button icon={<PlayCircleOutlined />} onClick={() => startAll()}>全部启动</Button>
          <Button icon={<PauseCircleOutlined />} onClick={() => stopAll()}>全部停止</Button>
          <Button icon={<ReloadOutlined />} onClick={() => restartAll()}>全部重启</Button>
        </Space>
      </div>

      <Card variant="outlined">
        <Table
          rowKey="id"
          columns={columns}
          dataSource={list}
          loading={loading}
          pagination={{ pageSize: 20, showSizeChanger: true }}
        />
      </Card>

      <Modal
        title={editing ? '编辑服务' : '新增服务'}
        open={modalOpen}
        onOk={handleSave}
        onCancel={() => setModalOpen(false)}
        okText="保存"
        cancelText="取消"
        width={600}
      >
        <Form form={form} layout="vertical">
          <Form.Item name="name" label="服务名称" rules={[{ required: true, message: '请输入名称' }]}>
            <Input placeholder="例如：测试服务A" />
          </Form.Item>
          <Form.Item name="description" label="描述">
            <Input.TextArea rows={2} placeholder="可选" />
          </Form.Item>
          <Form.Item name="protocol" label="协议类型" rules={[{ required: true }]}>
            <Select placeholder="选择协议">
              <Option value="websocket">WebSocket</Option>
              <Option value="socketio">Socket.IO</Option>
            </Select>
          </Form.Item>
          <Form.Item name="ip" label="监听IP" rules={[{ required: true }]}>
            <Input placeholder="0.0.0.0" />
          </Form.Item>
          <Form.Item name="port" label="监听端口" rules={[{ required: true }]}>
            <InputNumber min={1} max={65535} style={{ width: '100%' }} />
          </Form.Item>
          <Form.Item name="autoStart" label="自动启动" valuePropName="checked">
            <Select>
              <Option value={true}>是</Option>
              <Option value={false}>否</Option>
            </Select>
          </Form.Item>
          <Form.Item name="logLevel" label="日志等级">
            <Select>
              <Option value="DEBUG">DEBUG</Option>
              <Option value="INFO">INFO</Option>
              <Option value="WARN">WARN</Option>
              <Option value="ERROR">ERROR</Option>
            </Select>
          </Form.Item>
        </Form>
      </Modal>
    </div>
  );
}

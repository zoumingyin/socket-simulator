/**
 * EventManagerPage - 事件管理页面
 * 支持新增/编辑/删除/启用/禁用事件，同时支持 WebSocket 和 Socket.IO
 * 支持配置轮询：启用后按间隔自动广播默认消息
 */
import React, { useEffect, useState } from 'react';
import {
  Card, Button, Table, Tag, Space, Modal, Form, Input, InputNumber, Select,
  Switch, message, Popconfirm, Typography,
} from 'antd';
import {
  PlusOutlined, EditOutlined, DeleteOutlined,
  CheckCircleOutlined, StopOutlined,
} from '@ant-design/icons';
import type { EventConfig, EventStatus } from '../../types/index.js';
import { useEventStore } from '../../store/useEventStore.js';

const { Title } = Typography;
const { Option } = Select;
const { TextArea } = Input;

export function EventManagerPage(): React.ReactElement {
  const [messageApi, contextHolder] = message.useMessage();
  const { list, servers, loading, fetchEvents, fetchServers, addEvent, updateEvent, removeEvent, toggleEvent } = useEventStore();
  const [modalOpen, setModalOpen] = useState(false);
  const [editing, setEditing] = useState<EventConfig | null>(null);
  const [form] = Form.useForm();
  const [serverFilter, setServerFilter] = useState<string | undefined>(undefined);

  useEffect(() => { fetchServers(); }, []);
  useEffect(() => { fetchEvents(serverFilter); }, [serverFilter]);

  const handleSave = async () => {
    try {
      const vals = await form.validateFields();
      if (editing) {
        await updateEvent(editing.id, vals);
        messageApi.success('更新成功');
      } else {
        await addEvent(vals);
        messageApi.success('添加成功');
      }
      setModalOpen(false);
      form.resetFields();
      setEditing(null);
    } catch (e: unknown) {
      messageApi.error((e as Error).message || '保存失败');
    }
  };

  const columns = [
    { title: '事件名称', dataIndex: 'name', key: 'name', align: 'center' as const, ellipsis: true },
    { title: '服务ID', dataIndex: 'serverId', key: 'serverId', align: 'center' as const, ellipsis: true },
    { title: '描述', dataIndex: 'description', key: 'description', align: 'center' as const, ellipsis: true },
    { title: '默认事件', dataIndex: 'isDefault', key: 'isDefault',
      align: 'center' as const,
      render: (v: boolean) => v ? <Tag color="blue">默认</Tag> : <></>,
    },
    { title: '状态', dataIndex: 'status', key: 'status',
      align: 'center' as const,
      render: (v: string) => <Tag color={v === 'enabled' ? 'success' : 'default'}>{v}</Tag>,
    },
    { title: '轮询', dataIndex: 'pollingEnabled', key: 'pollingEnabled',
      align: 'center' as const,
      render: (v: boolean, r: EventConfig) => v ? <Tag color="blue">{r.pollingInterval}s</Tag> : <></>,
    },
    { title: '操作', key: 'actions', align: 'center' as const, render: (_: unknown, r: EventConfig) => (
        <Space>
          {r.status === 'enabled'
            ? <Button size="small" icon={<StopOutlined />} onClick={() => toggleEvent(r.id, 'disabled')}>禁用</Button>
            : <Button size="small" type="primary" icon={<CheckCircleOutlined />} onClick={() => toggleEvent(r.id, 'enabled')}>启用</Button>
          }
          {!r.isDefault && <Button size="small" icon={<EditOutlined />} onClick={() => { setEditing(r); form.setFieldsValue(r); setModalOpen(true); }}>编辑</Button>}
          {!r.isDefault && <Popconfirm title="确认删除？" onConfirm={() => removeEvent(r.id)}><Button size="small" danger icon={<DeleteOutlined />}>删除</Button></Popconfirm>}
        </Space>
      ),
    },
  ];

  return (
    <div>
      {contextHolder}
      <Title level={4} style={{ marginBottom: 16, fontSize: 18 }}>事件管理</Title>
      <Card variant="outlined">
        <Space style={{ marginBottom: 16 }} wrap>
          <Button type="primary" icon={<PlusOutlined />} onClick={() => { setEditing(null); form.resetFields(); setModalOpen(true); }}>新增事件</Button>
          <Select
            placeholder="筛选所属服务"
            allowClear
            style={{ width: 220 }}
            value={serverFilter}
            onChange={v => setServerFilter(v)}
          >
            {servers.map(s => (
              <Option key={s.id} value={s.id}>{s.name}（{s.id}）</Option>
            ))}
          </Select>
        </Space>
        <Table bordered rowKey="id" columns={columns} dataSource={list} loading={loading} pagination={{ pageSize: 15, showSizeChanger: true, showTotal: (total) => `共 ${total} 个事件` }} size="small" scroll={{ x: 'max-content' }} />
      </Card>

      <Modal
        title={editing ? '编辑事件' : '新增事件'}
        open={modalOpen}
        onOk={handleSave}
        onCancel={() => { setModalOpen(false); setEditing(null); }}
        width={560}
      >
        <Form form={form} layout="vertical" initialValues={{ status: 'enabled', isDefault: false, pollingEnabled: false, pollingInterval: 10 }}>
          <Form.Item name="serverId" label="所属服务" rules={[{ required: true }]}>
            <Select placeholder="请选择服务">
              {servers.map(s => (
                <Option key={s.id} value={s.id}>{s.name}（{s.id}）</Option>
              ))}
            </Select>
          </Form.Item>
          <Form.Item name="name" label="事件名称" rules={[{ required: true }]}><Input /></Form.Item>
          <Form.Item name="description" label="描述"><Input /></Form.Item>
          <Form.Item name="status" label="状态">
            <Select>
              <Option value="enabled">启用</Option>
              <Option value="disabled">禁用</Option>
            </Select>
          </Form.Item>
          {editing && <Form.Item name="isDefault" label="默认事件" valuePropName="checked"><Switch /></Form.Item>}

          <Form.Item name="pollingEnabled" label="启用轮询" valuePropName="checked"><Switch /></Form.Item>
          <Form.Item name="pollingInterval" label="轮询间隔（秒）" rules={[{ required: true, message: '请输入间隔秒数' }]}>
            <InputNumber min={1} max={86400} style={{ width: '100%' }} />
          </Form.Item>
          <Form.Item name="defaultMessage" label="默认消息内容（轮询时发送）">
            <TextArea rows={4} placeholder="JSON 或纯文本，轮询时自动发送此内容" />
          </Form.Item>
        </Form>
      </Modal>
    </div>
  );
}

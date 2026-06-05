/**
 * ClientManagerPage - 客户端管理页面
 */
import React, { useEffect, useState } from 'react';
import {
  Card, Table, Tag, Space, Button, Input, Modal, Form, message, Popconfirm, Typography,
} from 'antd';
import { SearchOutlined, SendOutlined, DisconnectOutlined } from '@ant-design/icons';
import type { ClientInfo } from '../../types/index.js';
import { useClientStore } from '../../store/useClientStore.js';

const { Title } = Typography;

export function ClientManagerPage(): React.ReactElement {
  const { list, loading, fetchClients, sendMessage, disconnectClient } = useClientStore();
  const [keyword, setKeyword] = useState('');
  const [serverFilter, setServerFilter] = useState<string | undefined>(undefined);
  const [sendModalOpen, setSendModalOpen] = useState(false);
  const [sendingTo, setSendingTo] = useState<ClientInfo | null>(null);
  const [sendForm] = Form.useForm();
  const [sending, setSending] = useState(false);

  useEffect(() => { fetchClients(); }, []);

  const filtered = list.filter(c => {
    if (serverFilter && c.serverId !== serverFilter) return false;
    if (keyword) {
      const kw = keyword.toLowerCase();
      return c.id.toLowerCase().includes(kw) || c.ipAddress.toLowerCase().includes(kw);
    }
    return true;
  });

  const handleSend = async () => {
    try {
      const values = await sendForm.validateFields();
      setSending(true);
      await sendMessage({
        serverId: sendingTo!.serverId,
        targetType: 'client',
        targetId: sendingTo!.id,
        event: values.event || 'message',
        messageType: 'text',
        content: values.content,
      });
      message.success('消息已发送');
      setSendModalOpen(false);
      sendForm.resetFields();
    } catch (err: unknown) {
      message.error('发送失败：' + ((err as Error).message || '未知错误'));
    } finally {
      setSending(false);
    }
  };

  const columns = [
    { title: '客户端ID', dataIndex: 'id', key: 'id', ellipsis: true, width: 160 },
    { title: 'SocketID', dataIndex: 'socketId', key: 'socketId', ellipsis: true, width: 160 },
    { title: 'IP地址', dataIndex: 'ipAddress', key: 'ipAddress', width: 140 },
    { title: '协议', dataIndex: 'protocol', key: 'protocol',
      render: (v: string) => <Tag color={v === 'websocket' ? 'blue' : 'green'}>{v}</Tag>,
    },
    { title: '状态', dataIndex: 'status', key: 'status',
      render: (v: string) => <Tag color={v === 'connected' ? 'success' : 'default'}>{v}</Tag>,
    },
    { title: '连接时间', dataIndex: 'connectedAt', key: 'connectedAt', width: 180 },
    { title: '最后活动', dataIndex: 'lastActivityAt', key: 'lastActivityAt', width: 180 },
    { title: '操作', key: 'actions', width: 180, render: (_: unknown, r: ClientInfo) => (
        <Space>
          <Button size="small" icon={<SendOutlined />} onClick={() => {
            setSendingTo(r);
            setSendModalOpen(true);
          }}>发消息</Button>
          <Popconfirm title="确认断开？" onConfirm={() => disconnectClient(r.serverId, r.id)}>
            <Button size="small" danger icon={<DisconnectOutlined />}>断开</Button>
          </Popconfirm>
        </Space>
      ),
    },
  ];

  return (
    <div>
      <Title level={4} style={{ marginBottom: 16 }}>客户端管理</Title>
      <Card variant="outlined">
        <Space style={{ marginBottom: 16 }}>
          <Input
            placeholder="搜索 ID / IP"
            prefix={<SearchOutlined />}
            value={keyword}
            onChange={e => setKeyword(e.target.value)}
            style={{ width: 240 }}
          />
        </Space>
        <Table rowKey="id" columns={columns} dataSource={filtered} loading={loading} pagination={{ pageSize: 20 }} size="small" />
      </Card>
      <Modal
        title={`发送消息给 ${sendingTo?.id ?? ''}`}
        open={sendModalOpen}
        onCancel={() => { setSendModalOpen(false); sendForm.resetFields(); }}
        onOk={handleSend}
        confirmLoading={sending}
        okText="发送"
        cancelText="取消"
      >
        <Form form={sendForm} layout="vertical">
          <Form.Item name="event" label="事件名" initialValue="message"
            rules={[{ required: true, message: '请输入事件名' }]}>
            <Input placeholder="客户端监听的事件名，如 message" />
          </Form.Item>
          <Form.Item name="content" label="消息内容"
            rules={[{ required: true, message: '请输入消息内容' }]}>
            <Input.TextArea rows={4} placeholder="输入要发送的消息内容" />
          </Form.Item>
        </Form>
      </Modal>
    </div>
  );
}

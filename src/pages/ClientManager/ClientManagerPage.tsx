/**
 * ClientManagerPage - 客户端管理页面（优化版）
 * 优化：改进搜索、添加分组视图、连接时间格式化、状态可视化
 */
import React, { useEffect, useState } from 'react';
import {
  Card, Table, Tag, Space, Button, Input, Modal, Form, message, Popconfirm, Typography, Spin,
  Select, Tooltip, Badge, Radio, Empty, Avatar,
} from 'antd';
import { SearchOutlined, SendOutlined, DisconnectOutlined, TeamOutlined, GroupOutlined } from '@ant-design/icons';
import type { ClientInfo, ClientGroup } from '../../types/index.js';
import { useClientStore } from '../../store/useClientStore.js';

const { Title, Text, Paragraph } = Typography;
const { Option } = Select;

type ViewMode = 'table' | 'grouped';

export function ClientManagerPage(): React.ReactElement {
  const { list, groups, loading, fetchClients, fetchGroups, sendMessage, disconnectClient } = useClientStore();
  const [messageApi, contextHolder] = message.useMessage();
  
  const [keyword, setKeyword] = useState('');
  const [serverFilter, setServerFilter] = useState<string | undefined>(undefined);
  const [statusFilter, setStatusFilter] = useState<'all' | 'connected' | 'disconnected'>('all');
  
  const [sendModalOpen, setSendModalOpen] = useState(false);
  const [sendingTo, setSendingTo] = useState<ClientInfo | null>(null);
  const [sendForm] = Form.useForm();
  const [sending, setSending] = useState(false);
  
  const [viewMode, setViewMode] = useState<ViewMode>('table');

  useEffect(() => { fetchClients(); }, []);

  // 筛选逻辑
  const filtered = list.filter(c => {
    if (serverFilter && c.serverId !== serverFilter) return false;
    if (statusFilter !== 'all' && c.status !== statusFilter) return false;
    if (keyword) {
      const kw = keyword.toLowerCase();
      return c.id.toLowerCase().includes(kw) || 
             c.socketId.toLowerCase().includes(kw) || 
             c.ipAddress.toLowerCase().includes(kw);
    }
    return true;
  });

  // 按服务分组
  const groupedByServer = list.reduce((acc, client) => {
    const key = client.serverId;
    if (!acc[key]) acc[key] = [];
    acc[key].push(client);
    return acc;
  }, {} as Record<string, ClientInfo[]>);

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
      messageApi.success('消息已发送');
      setSendModalOpen(false);
      sendForm.resetFields();
    } catch (err: unknown) {
      messageApi.error('发送失败：' + ((err as Error).message || '未知错误'));
    } finally {
      setSending(false);
    }
  };

  // 格式化时间
  const formatTime = (timeStr: string) => {
    if (!timeStr) return '-';
    const date = new Date(timeStr);
    return date.toLocaleString('zh-CN', { hour12: false });
  };

  // 表格列定义
  const columns = [
    {
      title: '状态',
      dataIndex: 'status',
      key: 'status',
      align: 'center' as const,
      filters: [
        { text: '已连接', value: 'connected' },
        { text: '已断开', value: 'disconnected' },
      ],
      onFilter: (value: boolean | React.Key, record: ClientInfo) => record.status === String(value),
      render: (v: string) => (
        <Badge 
          status={v === 'connected' ? 'success' : 'default'} 
          text={v === 'connected' ? '在线' : '离线'} 
        />
      ),
    },
    {
      title: '客户端ID',
      dataIndex: 'id',
      key: 'id',
      align: 'center' as const,
      ellipsis: true,
      render: (v: string) => <Tooltip title={v}><Text code>{v}</Text></Tooltip>,
    },
    {
      title: 'Socket ID',
      dataIndex: 'socketId',
      key: 'socketId',
      align: 'center' as const,
      ellipsis: true,
      render: (v: string) => <Text type="secondary">{v}</Text>,
    },
    {
      title: 'IP地址',
      dataIndex: 'ipAddress',
      key: 'ipAddress',
      align: 'center' as const,
      render: (v: string) => <Tag color="blue">{v}</Tag>,
    },
    {
      title: '协议',
      dataIndex: 'protocol',
      key: 'protocol',
      align: 'center' as const,
      render: (v: string) => <Tag color={v === 'websocket' ? 'blue' : 'green'}>{v.toUpperCase()}</Tag>,
    },
    {
      title: '分组',
      dataIndex: 'group',
      key: 'group',
      align: 'center' as const,
      ellipsis: true,
      render: (v: string) => v ? <Tag>{v}</Tag> : '-',
    },
    {
      title: '连接时间',
      dataIndex: 'connectedAt',
      key: 'connectedAt',
      align: 'center' as const,
      ellipsis: true,
      render: (v: string) => <Text type="secondary">{formatTime(v)}</Text>,
    },
    {
      title: '最后活动',
      dataIndex: 'lastActivityAt',
      key: 'lastActivityAt',
      align: 'center' as const,
      ellipsis: true,
      render: (v: string) => <Text type="secondary">{formatTime(v)}</Text>,
    },
    {
      title: '操作',
      key: 'actions',
      align: 'center' as const,
      render: (_: unknown, r: ClientInfo) => (
        <Space>
          <Tooltip title="发送消息">
            <Button 
              size="small" 
              icon={<SendOutlined />} 
              onClick={() => {
                setSendingTo(r);
                setSendModalOpen(true);
              }}
              disabled={r.status !== 'connected'}
            >发消息</Button>
          </Tooltip>
          <Popconfirm title="确认断开？" onConfirm={() => disconnectClient(r.serverId, r.id)}>
            <Tooltip title="断开连接">
              <Button size="small" danger icon={<DisconnectOutlined />}>断开</Button>
            </Tooltip>
          </Popconfirm>
        </Space>
      ),
    },
  ];

  return (
    <div>
      {contextHolder}
      
      {/* 标题栏 */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 12 }}>
        <div style={{ display: 'flex', alignItems: 'center' }}>
          <TeamOutlined style={{ fontSize: 20, marginRight: 8, color: '#1677ff' }} />
          <Title level={4} style={{ margin: 0, fontSize: 18 }}>客户端管理</Title>
          {loading && <Spin size="small" style={{ marginLeft: 12 }} />}
        </div>
        <Space>
          <Tooltip title="视图切换">
            <Radio.Group
              value={viewMode}
              onChange={(e) => setViewMode(e.target.value)}
              size="small"
              optionType="button"
              buttonStyle="solid"
            >
              <Radio.Button value="table"><TeamOutlined /> 列表</Radio.Button>
              <Radio.Button value="grouped"><GroupOutlined /> 分组</Radio.Button>
            </Radio.Group>
          </Tooltip>
        </Space>
      </div>

      {/* 筛选栏 */}
      <Card size="small" style={{ marginBottom: 16 }}>
        <Space wrap>
          <Input
            placeholder="搜索 ID / Socket ID / IP"
            prefix={<SearchOutlined />}
            value={keyword}
            onChange={e => setKeyword(e.target.value)}
            style={{ width: 280 }}
            allowClear
          />
          <Select
            placeholder="按服务筛选"
            value={serverFilter}
            onChange={setServerFilter}
            style={{ width: 180 }}
            allowClear
          >
            {Array.from(new Set(list.map(c => c.serverId))).map(sid => (
              <Option key={sid} value={sid}>{sid}</Option>
            ))}
          </Select>
          <Radio.Group
            value={statusFilter}
            onChange={(e) => setStatusFilter(e.target.value)}
            size="small"
            optionType="button"
            buttonStyle="solid"
          >
            <Radio.Button value="all">全部 ({list.length})</Radio.Button>
            <Radio.Button value="connected">
              <Badge status="success" /> 在线 ({list.filter(c => c.status === 'connected').length})
            </Radio.Button>
            <Radio.Button value="disconnected">
              <Badge status="default" /> 离线 ({list.filter(c => c.status === 'disconnected').length})
            </Radio.Button>
          </Radio.Group>
        </Space>
      </Card>

      {/* 统计信息 */}
      <div style={{ marginBottom: 16, display: 'flex', gap: 16 }}>
        <Text>总客户端: <Text strong>{list.length}</Text></Text>
        <Text>|</Text>
        <Text>在线: <Text strong style={{ color: '#52c41a' }}>{list.filter(c => c.status === 'connected').length}</Text></Text>
        <Text>|</Text>
        <Text>离线: <Text strong style={{ color: '#999' }}>{list.filter(c => c.status !== 'connected').length}</Text></Text>
      </div>

      {/* 内容区域 */}
      {viewMode === 'table' ? (
        <Card variant="outlined">
          <Table
            bordered
            rowKey="id"
            columns={columns}
            dataSource={filtered}
            loading={loading}
            pagination={{ pageSize: 15, showSizeChanger: true, showTotal: (total) => `共 ${total} 个客户端` }}
            size="small"
            scroll={{ x: 'max-content' }}
          />
        </Card>
      ) : (
        <div>
          {Object.keys(groupedByServer).length === 0 ? (
            <Empty description="暂无客户端连接" />
          ) : (
            Object.entries(groupedByServer).map(([serverId, clients]) => (
              <Card
                key={serverId}
                title={
                  <div style={{ display: 'flex', alignItems: 'center' }}>
                    <Avatar icon={<TeamOutlined />} style={{ backgroundColor: '#1677ff', marginRight: 8 }} size="small" />
                    <Text strong>{serverId}</Text>
                    <Badge 
                      count={clients.filter(c => c.status === 'connected').length} 
                      style={{ marginLeft: 8 }}
                      title="在线客户端数"
                    />
                  </div>
                }
                style={{ marginBottom: 16 }}
                size="small"
              >
                <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(250px, 1fr))', gap: 8 }}>
                  {clients.map(client => (
                    <Card
                      key={client.id}
                      size="small"
                      hoverable
                      style={{ 
                        borderLeft: `3px solid ${client.status === 'connected' ? '#52c41a' : '#d9d9d9'}`,
                        padding: 8
                      }}
                    >
                      <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 4 }}>
                        <Text strong><Tag color={client.protocol === 'websocket' ? 'blue' : 'green'}>{client.protocol}</Tag></Text>
                        <Badge status={client.status === 'connected' ? 'success' : 'default'} />
                      </div>
                      <div style={{ fontSize: 12, color: '#666', marginBottom: 4 }}>
                        <div>IP: {client.ipAddress}</div>
                        <div>连接: {formatTime(client.connectedAt)}</div>
                      </div>
                      <Space size="small">
                        <Tooltip title="发送消息">
                          <Button 
                            size="small" 
                            icon={<SendOutlined />} 
                            onClick={() => {
                              setSendingTo(client);
                              setSendModalOpen(true);
                            }}
                            disabled={client.status !== 'connected'}
                          />
                        </Tooltip>
                        <Popconfirm title="确认断开？" onConfirm={() => disconnectClient(client.serverId, client.id)}>
                          <Tooltip title="断开连接">
                            <Button size="small" danger icon={<DisconnectOutlined />} />
                          </Tooltip>
                        </Popconfirm>
                      </Space>
                    </Card>
                  ))}
                </div>
              </Card>
            ))
          )}
        </div>
      )}

      {/* 发送消息弹窗 */}
      <Modal
        title={`发送消息给 ${sendingTo?.id ?? ''}`}
        open={sendModalOpen}
        forceRender
        onCancel={() => { setSendModalOpen(false); sendForm.resetFields(); }}
        onOk={handleSend}
        confirmLoading={sending}
        okText="发送"
        cancelText="取消"
      >
        <Form form={sendForm} layout="vertical">
          <Form.Item 
            name="event" 
            label="事件名" 
            initialValue="message"
            rules={[{ required: true, message: '请输入事件名' }]}
            tooltip="客户端监听的事件名"
          >
            <Input placeholder="客户端监听的事件名，如 message" />
          </Form.Item>
          <Form.Item 
            name="content" 
            label="消息内容"
            rules={[{ required: true, message: '请输入消息内容' }]}
          >
            <Input.TextArea rows={4} placeholder="输入要发送的消息内容" />
          </Form.Item>
        </Form>
      </Modal>
    </div>
  );
}

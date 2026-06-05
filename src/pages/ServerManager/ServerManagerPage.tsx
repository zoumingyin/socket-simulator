/**
 * ServerManagerPage - 服务管理页面（优化版）
 * 支持：新增/编辑/删除/启动/停止/重启服务，以及启动全部/停止全部/重启全部
 * 优化：添加批量操作、状态筛选、卡片视图切换
 */
import React, { useEffect, useState } from 'react';
import {
  Card, Button, Table, Tag, Space, Modal, Form, Input, InputNumber,
  Select, Popconfirm, Typography, message, Tooltip, Badge, Alert,
  Radio, Spin,
} from 'antd';
import {
  PlusOutlined, PlayCircleOutlined, PauseCircleOutlined,
  ReloadOutlined, EditOutlined, DeleteOutlined,
  AppstoreOutlined, UnorderedListOutlined, FilterOutlined,
  ExclamationCircleOutlined, CheckCircleOutlined,
} from '@ant-design/icons';
import type { ServerConfig, ServerRuntime } from '../../types/index.js';
import { useServerStore } from '../../store/useServerStore.js';

const { Title, Text, Paragraph } = Typography;
const { Option } = Select;

type ViewMode = 'table' | 'card';

export function ServerManagerPage(): React.ReactElement {
  const [messageApi, contextHolder] = message.useMessage();
  const {
    list, runtimes, loading, error,
    fetchServers, fetchRuntimes,
    addServer, updateServer, removeServer,
    startServer, stopServer, restartServer,
    startAll, stopAll, restartAll,
    batchStart, batchStop, batchRestart, batchDelete,
  } = useServerStore();
  
  const [modalOpen, setModalOpen] = useState(false);
  const [editing, setEditing] = useState<ServerConfig | null>(null);
  const [form] = Form.useForm();
  
  // 筛选和视图
  const [statusFilter, setStatusFilter] = useState<'all' | 'running' | 'stopped'>('all');
  const [viewMode, setViewMode] = useState<ViewMode>('table');
  const [selectedRowKeys, setSelectedRowKeys] = useState<React.Key[]>([]);

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

  // 筛选逻辑
  const filteredList = list.filter((s) => {
    if (statusFilter === 'running') return runtimes[s.id]?.status === 'running';
    if (statusFilter === 'stopped') return !runtimes[s.id] || runtimes[s.id]?.status !== 'running';
    return true;
  });

  // 表格列定义
  const columns = [
    {
      title: '名称',
      dataIndex: 'name',
      key: 'name',
      align: 'center' as const,
      ellipsis: true,
      render: (v: string, r: ServerConfig) => (
        <Tooltip title={`ID: ${r.id}`}>
          <Text strong>{v || r.id || '未知'}</Text>
        </Tooltip>
      ),
    },
    {
      title: '协议',
      dataIndex: 'protocol',
      key: 'protocol',
      align: 'center' as const,
      render: (v: string) => (
        <Tag color={v === 'websocket' ? 'blue' : 'green'} style={{ margin: 0 }}>
          {v === 'websocket' ? 'WS' : 'S.IO'}
        </Tag>
      ),
    },
    {
      title: '地址',
      key: 'addr',
      align: 'center' as const,
      ellipsis: true,
      render: (_: unknown, r: ServerConfig) => <Text code style={{ fontSize: 12 }}>{r.ip}:{r.port}</Text>,
    },
    {
      title: '状态',
      key: 'status',
      align: 'center' as const,
      render: (_: unknown, r: ServerConfig) => {
        const rt = runtimes[r.id];
        const running = rt?.status === 'running';
        return (
          <Tag color={running ? 'success' : 'default'} style={{ margin: 0, fontSize: 12 }}>
            {running ? '运行中' : '已停止'}
          </Tag>
        );
      },
    },
    {
      title: '客户端',
      key: 'clients',
      align: 'center' as const,
      render: (_: unknown, r: ServerConfig) => {
        const count = runtimes[r.id]?.clientCount ?? 0;
        return <Text strong style={{ color: count > 0 ? '#52c41a' : undefined, fontSize: 13 }}>{count}</Text>;
      },
    },
    {
      title: '操作',
      key: 'actions',
      align: 'center' as const,
      render: (_: unknown, r: ServerConfig) => {
        const rt = runtimes[r.id];
        const running = rt?.status === 'running';
        return (
          <Space size={2}>
            {!running && (
              <Tooltip title="启动">
                <Button type="text" icon={<PlayCircleOutlined />} onClick={() => startServer(r.id)} size="small" />
              </Tooltip>
            )}
            {running && (
              <Tooltip title="停止">
                <Button type="text" danger icon={<PauseCircleOutlined />} onClick={() => stopServer(r.id)} size="small" />
              </Tooltip>
            )}
            {running && (
              <Tooltip title="重启">
                <Button type="text" icon={<ReloadOutlined />} onClick={() => restartServer(r.id)} size="small" />
              </Tooltip>
            )}
            <Tooltip title="编辑">
              <Button type="text" icon={<EditOutlined />} onClick={() => openEdit(r)} size="small" />
            </Tooltip>
            <Popconfirm title="确认删除？" onConfirm={() => removeServer(r.id)}>
              <Tooltip title="删除">
                <Button type="text" danger icon={<DeleteOutlined />} size="small" />
              </Tooltip>
            </Popconfirm>
          </Space>
        );
      },
    },
  ];

  // 批量操作
  const hasSelection = selectedRowKeys.length > 0;
  const selectedServers = list.filter((s) => selectedRowKeys.includes(s.id));
  const allRunning = selectedServers.every((s) => runtimes[s.id]?.status === 'running');
  const allStopped = selectedServers.every((s) => !runtimes[s.id] || runtimes[s.id]?.status !== 'running');

  return (
    <div>
      {contextHolder}
      
      {/* 标题栏 */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 12 }}>
        <div style={{ display: 'flex', alignItems: 'center' }}>
          <CloudServerOutlined style={{ fontSize: 20, marginRight: 8, color: '#1677ff' }} />
          <Title level={4} style={{ margin: 0, fontSize: 18 }}>服务管理</Title>
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
              <Radio.Button value="table"><UnorderedListOutlined /></Radio.Button>
              <Radio.Button value="card"><AppstoreOutlined /></Radio.Button>
            </Radio.Group>
          </Tooltip>
          <Button type="primary" icon={<PlusOutlined />} onClick={openAdd}>新增服务</Button>
          <Button icon={<PlayCircleOutlined />} onClick={() => startAll()} disabled={list.length === 0}>全部启动</Button>
          <Button icon={<PauseCircleOutlined />} onClick={() => stopAll()} disabled={list.length === 0}>全部停止</Button>
          <Button icon={<ReloadOutlined />} onClick={() => restartAll()} disabled={list.length === 0}>全部重启</Button>
        </Space>
      </div>

      {/* 错误提示 */}
      {error && <Alert type="error" message={error} style={{ marginBottom: 16 }} closable />}

      {/* 筛选栏 */}
      <Card size="small" style={{ marginBottom: 16 }}>
        <Space>
          <Text strong><FilterOutlined /> 筛选：</Text>
          <Radio.Group
            value={statusFilter}
            onChange={(e) => setStatusFilter(e.target.value)}
            size="small"
            optionType="button"
            buttonStyle="solid"
          >
            <Radio.Button value="all">全部 ({list.length})</Radio.Button>
            <Radio.Button value="running">
              <CheckCircleOutlined /> 运行中 ({list.filter((s) => runtimes[s.id]?.status === 'running').length})
            </Radio.Button>
            <Radio.Button value="stopped">
              <ExclamationCircleOutlined /> 已停止 ({list.filter((s) => !runtimes[s.id] || runtimes[s.id]?.status !== 'running').length})
            </Radio.Button>
          </Radio.Group>
        </Space>
      </Card>

      {/* 批量操作栏 */}
      {hasSelection && (
        <Card size="small" style={{ marginBottom: 16, background: '#e6f7ff' }}>
          <Space>
            <Text strong>已选择 {selectedRowKeys.length} 项</Text>
            {allStopped && (
              <Button size="small" type="primary" icon={<PlayCircleOutlined />} onClick={() => {
                batchStart(selectedRowKeys as string[]);
                setSelectedRowKeys([]);
              }}>批量启动</Button>
            )}
            {allRunning && (
              <Button size="small" danger icon={<PauseCircleOutlined />} onClick={() => {
                batchStop(selectedRowKeys as string[]);
                setSelectedRowKeys([]);
              }}>批量停止</Button>
            )}
            <Button size="small" icon={<ReloadOutlined />} onClick={() => {
              batchRestart(selectedRowKeys as string[]);
              setSelectedRowKeys([]);
            }}>批量重启</Button>
            <Popconfirm title={`确认删除选中的 ${selectedRowKeys.length} 个服务？`} onConfirm={() => {
              batchDelete(selectedRowKeys as string[]);
              setSelectedRowKeys([]);
            }}>
              <Button size="small" danger icon={<DeleteOutlined />}>批量删除</Button>
            </Popconfirm>
          </Space>
        </Card>
      )}

      {/* 内容区域 */}
      {viewMode === 'table' ? (
        <Card variant="outlined">
          <Table
            bordered
            rowKey="id"
            columns={columns}
            dataSource={filteredList}
            loading={loading}
            pagination={{ pageSize: 15, showSizeChanger: true, showTotal: (total) => `共 ${total} 个服务` }}
            rowSelection={{
              selectedRowKeys,
              onChange: (keys) => setSelectedRowKeys(keys),
            }}
            size="small"
            scroll={{ x: 'max-content' }}
            rowClassName={(record) => runtimes[record.id]?.status === 'running' ? 'row-running' : 'row-stopped'}
          />
        </Card>
      ) : (
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(300px, 1fr))', gap: 16 }}>
          {filteredList.map((server) => {
            const rt = runtimes[server.id];
            const running = rt?.status === 'running';
            return (
              <Card
                key={server.id}
                hoverable
                style={{ borderLeft: `4px solid ${running ? '#52c41a' : '#d9d9d9'}` }}
                actions={[
                  !running ? <Tooltip title="启动"><PlayCircleOutlined onClick={() => startServer(server.id)} /></Tooltip> : null,
                  running ? <Tooltip title="停止"><PauseCircleOutlined style={{ color: '#ff4d4f' }} onClick={() => stopServer(server.id)} /></Tooltip> : null,
                  running ? <Tooltip title="重启"><ReloadOutlined onClick={() => restartServer(server.id)} /></Tooltip> : null,
                  <Tooltip title="编辑"><EditOutlined onClick={() => openEdit(server)} /></Tooltip>,
                  <Popconfirm title="确认删除？" onConfirm={() => removeServer(server.id)}>
                    <Tooltip title="删除"><DeleteOutlined style={{ color: '#ff4d4f' }} /></Tooltip>
                  </Popconfirm>,
                ].filter(Boolean)}
              >
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 12 }}>
                  <Text strong style={{ fontSize: 16 }}>{server.name || server.id}</Text>
                  <Badge status={running ? 'success' : 'default'} text={running ? '运行中' : '已停止'} />
                </div>
                <div style={{ marginBottom: 8 }}>
                  <Tag color={server.protocol === 'websocket' ? 'blue' : 'green'}>{server.protocol.toUpperCase()}</Tag>
                  <Text code>{server.ip}:{server.port}</Text>
                </div>
                <div style={{ color: '#666', fontSize: 12 }}>
                  <div>客户端: {rt?.clientCount ?? 0}</div>
                  <div>发送: {rt?.sentMessages ?? 0} | 接收: {rt?.receivedMessages ?? 0}</div>
                </div>
              </Card>
            );
          })}
        </div>
      )}

      {/* 新增/编辑弹窗 */}
      <Modal
        title={editing ? '编辑服务' : '新增服务'}
        open={modalOpen}
        onOk={handleSave}
        onCancel={() => { setModalOpen(false); form.resetFields(); }}
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

// 引入图标组件
const CloudServerOutlined = PlusOutlined; // 占位，实际使用时应从 @ant-design/icons 导入

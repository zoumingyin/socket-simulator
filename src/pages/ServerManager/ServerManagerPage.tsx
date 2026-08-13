/**
 * ServerManagerPage - 服务管理页面（重新设计）
 *
 * 左右分栏布局：
 * - 左侧：服务列表（表格/卡片可切换）+ 筛选 + 批量操作
 * - 右侧：详情面板（Tabs：服务配置 | Mock规则 | 接口测试）
 *
 * 兼容 Socket 服务（WS/SIO/HTTP）和统一路由模式（Mock + Socket 共端口）。
 */
import React, { useEffect, useState } from 'react';
import {
  Card, Button, Table, Tag, Space, Modal, Form, Input, InputNumber,
  Select, Popconfirm, Typography, message, Tooltip, Badge, Alert,
  Radio, Spin, Switch, Divider, Empty, Tabs,
} from 'antd';
import {
  PlusOutlined, PlayCircleOutlined, PauseCircleOutlined,
  ReloadOutlined, EditOutlined, DeleteOutlined,
  AppstoreOutlined, UnorderedListOutlined, FilterOutlined,
  ExclamationCircleOutlined, CheckCircleOutlined,
  CloudServerOutlined, ApiOutlined, ThunderboltOutlined,
} from '@ant-design/icons';
import type { ServerConfig, ServerRuntime, MockRule, HttpMethod } from '../../types/index.js';
import { useServerStore } from '../../store/useServerStore.js';
import {
  JsonEditor, MockRulesTable, TEST_METHODS,
} from '../../components/MockComponents.js';

const { Title, Text } = Typography;
const { Option } = Select;
const { TextArea } = Input;

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

  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [createOpen, setCreateOpen] = useState(false);
  const [createForm] = Form.useForm();
  const [statusFilter, setStatusFilter] = useState<'all' | 'running' | 'stopped'>('all');
  const [viewMode, setViewMode] = useState<ViewMode>('table');
  const [selectedRowKeys, setSelectedRowKeys] = useState<React.Key[]>([]);
  const [tab, setTab] = useState('config');

  useEffect(() => { fetchServers(); fetchRuntimes(); }, []);

  // 默认选中第一条
  useEffect(() => {
    if (!selectedId && list.length > 0) setSelectedId(list[0].id);
  }, [list, selectedId]);

  const selected = list.find((s) => s.id === selectedId) || null;

  // 筛选
  const filteredList = list.filter((s) => {
    if (statusFilter === 'running') return runtimes[s.id]?.status === 'running';
    if (statusFilter === 'stopped') return !runtimes[s.id] || runtimes[s.id]?.status !== 'running';
    return true;
  });

  // 新建服务
  const handleCreate = async () => {
    const vals = await createForm.validateFields();
    try {
      const created = await addServer({
        name: vals.name,
        description: vals.description || '',
        ip: vals.ip || '0.0.0.0',
        port: vals.port,
        protocol: vals.protocol,
        autoStart: vals.autoStart ?? false,
        logLevel: vals.logLevel || 'INFO',
        wssEnabled: false,
        httpRoutes: [],
        mockEnabled: false,
        mockRules: [],
        mockDefaultStatusCode: 200,
        mockDefaultResponseBody: '{"message":"ok"}',
        mockDefaultDelayMs: 0,
      });
      setSelectedId(created.id);
      setTab('config');
      setCreateOpen(false);
      createForm.resetFields();
      messageApi.success('已创建，请在右侧编辑详细配置');
    } catch (e) {
      messageApi.error('创建失败：' + (e as Error).message);
    }
  };

  // 保存配置
  const handleSaveConfig = async (vals: Record<string, unknown>) => {
    if (!selected) return;
    try {
      await updateServer(selected.id, vals as Partial<ServerConfig>);
      messageApi.success('配置已保存');
    } catch (e) {
      messageApi.error('保存失败：' + (e as Error).message);
    }
  };

  // 更新 Mock 规则
  const handleUpdateMockRules = async (rules: MockRule[]) => {
    if (!selected) return;
    await updateServer(selected.id, { mockRules: rules });
  };

  // 批量操作
  const hasSelection = selectedRowKeys.length > 0;
  const selectedServers = list.filter((s) => selectedRowKeys.includes(s.id));
  const allRunning = selectedServers.every((s) => runtimes[s.id]?.status === 'running');
  const allStopped = selectedServers.every((s) => !runtimes[s.id] || runtimes[s.id]?.status !== 'running');

  // 表格列
  const columns = [
    {
      title: '名称', dataIndex: 'name', key: 'name', ellipsis: true,
      render: (v: string, r: ServerConfig) => (
        <Tooltip title={`ID: ${r.id}`}>
          <Text strong style={{ cursor: 'pointer' }} onClick={() => { setSelectedId(r.id); setTab('config'); }}>
            {v || r.id || '未知'}
          </Text>
        </Tooltip>
      ),
    },
    {
      title: '协议', dataIndex: 'protocol', key: 'protocol', align: 'center' as const, width: 70,
      render: (v: string) => (
        <Tag color={v === 'websocket' ? 'blue' : v === 'http' ? 'purple' : 'green'} style={{ margin: 0 }}>
          {v === 'websocket' ? 'WS' : v === 'http' ? 'HTTP' : 'S.IO'}
        </Tag>
      ),
    },
    {
      title: 'Mock', key: 'mock', align: 'center' as const, width: 55,
      render: (_: unknown, r: ServerConfig) =>
        r.mockEnabled ? <Tag color="orange" style={{ margin: 0, fontSize: 11 }}>Mock</Tag> : <Text type="secondary" style={{ fontSize: 11 }}>-</Text>,
    },
    {
      title: '地址', key: 'addr', align: 'center' as const, ellipsis: true,
      render: (_: unknown, r: ServerConfig) => <Text code style={{ fontSize: 12 }}>{r.ip}:{r.port}</Text>,
    },
    {
      title: '状态', key: 'status', align: 'center' as const, width: 80,
      render: (_: unknown, r: ServerConfig) => {
        const running = runtimes[r.id]?.status === 'running';
        return <Tag color={running ? 'success' : 'default'} style={{ margin: 0, fontSize: 12 }}>{running ? '运行中' : '已停止'}</Tag>;
      },
    },
    {
      title: '客户端', key: 'clients', align: 'center' as const, width: 70,
      render: (_: unknown, r: ServerConfig) => {
        const count = runtimes[r.id]?.clientCount ?? 0;
        return <Text strong style={{ color: count > 0 ? '#52c41a' : undefined, fontSize: 13 }}>{count}</Text>;
      },
    },
    {
      title: '操作', key: 'actions', align: 'center' as const, width: 120,
      render: (_: unknown, r: ServerConfig) => {
        const running = runtimes[r.id]?.status === 'running';
        return (
          <Space size={2}>
            {!running && <Tooltip title="启动"><Button type="text" icon={<PlayCircleOutlined />} onClick={() => startServer(r.id)} size="small" /></Tooltip>}
            {running && <Tooltip title="停止"><Button type="text" danger icon={<PauseCircleOutlined />} onClick={() => stopServer(r.id)} size="small" /></Tooltip>}
            {running && <Tooltip title="重启"><Button type="text" icon={<ReloadOutlined />} onClick={() => restartServer(r.id)} size="small" /></Tooltip>}
            <Popconfirm title="确认删除？" onConfirm={() => { removeServer(r.id); if (selectedId === r.id) setSelectedId(null); }}>
              <Tooltip title="删除"><Button type="text" danger icon={<DeleteOutlined />} size="small" /></Tooltip>
            </Popconfirm>
          </Space>
        );
      },
    },
  ];

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
      {contextHolder}

      {/* 标题栏 */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 12 }}>
        <Space>
          <CloudServerOutlined style={{ fontSize: 20, color: '#1677ff' }} />
          <Title level={4} style={{ margin: 0, fontSize: 18 }}>服务管理</Title>
          {loading && <Spin size="small" />}
        </Space>
        <Space>
          <Button type="primary" icon={<PlusOutlined />} onClick={() => { createForm.resetFields(); setCreateOpen(true); }}>新增服务</Button>
          <Button icon={<PlayCircleOutlined />} onClick={() => startAll()} disabled={list.length === 0}>全部启动</Button>
          <Button icon={<PauseCircleOutlined />} onClick={() => stopAll()} disabled={list.length === 0}>全部停止</Button>
          <Button icon={<ReloadOutlined />} onClick={() => restartAll()} disabled={list.length === 0}>全部重启</Button>
        </Space>
      </div>

      {error && <Alert type="error" message={error} style={{ marginBottom: 12 }} closable />}

      {/* 主体：左右分栏 */}
      <div style={{ display: 'flex', flex: 1, gap: 12, minHeight: 0 }}>
        {/* 左侧：列表 */}
        <div style={{ width: '55%', display: 'flex', flexDirection: 'column', minHeight: 0 }}>
          {/* 筛选栏 */}
          <Card size="small" style={{ marginBottom: 8 }}>
            <Space>
              <Text strong><FilterOutlined /> 筛选：</Text>
              <Radio.Group value={statusFilter} onChange={(e) => setStatusFilter(e.target.value)} size="small" optionType="button" buttonStyle="solid">
                <Radio.Button value="all">全部 ({list.length})</Radio.Button>
                <Radio.Button value="running"><CheckCircleOutlined /> 运行中 ({list.filter((s) => runtimes[s.id]?.status === 'running').length})</Radio.Button>
                <Radio.Button value="stopped"><ExclamationCircleOutlined /> 已停止 ({list.filter((s) => !runtimes[s.id] || runtimes[s.id]?.status !== 'running').length})</Radio.Button>
              </Radio.Group>
              <Radio.Group value={viewMode} onChange={(e) => setViewMode(e.target.value)} size="small" optionType="button" buttonStyle="solid">
                <Radio.Button value="table"><UnorderedListOutlined /></Radio.Button>
                <Radio.Button value="card"><AppstoreOutlined /></Radio.Button>
              </Radio.Group>
            </Space>
          </Card>

          {/* 批量操作栏 */}
          {hasSelection && (
            <Card size="small" style={{ marginBottom: 8, background: '#e6f7ff' }}>
              <Space>
                <Text strong>已选择 {selectedRowKeys.length} 项</Text>
                {allStopped && <Button size="small" type="primary" icon={<PlayCircleOutlined />} onClick={() => { batchStart(selectedRowKeys as string[]); setSelectedRowKeys([]); }}>批量启动</Button>}
                {allRunning && <Button size="small" danger icon={<PauseCircleOutlined />} onClick={() => { batchStop(selectedRowKeys as string[]); setSelectedRowKeys([]); }}>批量停止</Button>}
                <Button size="small" icon={<ReloadOutlined />} onClick={() => { batchRestart(selectedRowKeys as string[]); setSelectedRowKeys([]); }}>批量重启</Button>
                <Popconfirm title={`确认删除选中的 ${selectedRowKeys.length} 个服务？`} onConfirm={() => { batchDelete(selectedRowKeys as string[]); setSelectedRowKeys([]); }}>
                  <Button size="small" danger icon={<DeleteOutlined />}>批量删除</Button>
                </Popconfirm>
              </Space>
            </Card>
          )}

          {/* 列表 */}
          {viewMode === 'table' ? (
            <Card variant="outlined" style={{ flex: 1, overflow: 'auto' }}>
              <Table
                bordered
                rowKey="id"
                columns={columns}
                dataSource={filteredList}
                loading={loading}
                pagination={{ pageSize: 15, showSizeChanger: true, showTotal: (total) => `共 ${total} 个服务` }}
                rowSelection={{ selectedRowKeys, onChange: (keys) => setSelectedRowKeys(keys) }}
                size="small"
                scroll={{ x: 'max-content' }}
                onRow={(r) => ({ onClick: () => { setSelectedId(r.id); setTab('config'); }, style: { cursor: 'pointer' } })}
                rowClassName={(record) => selectedId === record.id ? 'ant-table-row-selected' : (runtimes[record.id]?.status === 'running' ? 'row-running' : 'row-stopped')}
              />
            </Card>
          ) : (
            <div style={{ flex: 1, overflow: 'auto', display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(260px, 1fr))', gap: 12, alignContent: 'start' }}>
              {filteredList.map((server) => {
                const rt = runtimes[server.id];
                const running = rt?.status === 'running';
                return (
                  <Card
                    key={server.id}
                    hoverable
                    size="small"
                    onClick={() => { setSelectedId(server.id); setTab('config'); }}
                    style={{
                      borderLeft: `4px solid ${running ? '#52c41a' : '#d9d9d9'}`,
                      borderColor: selectedId === server.id ? '#1890ff' : undefined,
                      background: selectedId === server.id ? '#e6f7ff' : undefined,
                    }}
                    actions={[
                      !running ? <PlayCircleOutlined onClick={(e) => { e.stopPropagation(); startServer(server.id); }} /> : null,
                      running ? <PauseCircleOutlined style={{ color: '#ff4d4f' }} onClick={(e) => { e.stopPropagation(); stopServer(server.id); }} /> : null,
                      running ? <ReloadOutlined onClick={(e) => { e.stopPropagation(); restartServer(server.id); }} /> : null,
                      <Popconfirm title="确认删除？" onConfirm={() => { removeServer(server.id); if (selectedId === server.id) setSelectedId(null); }}>
                        <DeleteOutlined style={{ color: '#ff4d4f' }} onClick={(e) => e.stopPropagation()} />
                      </Popconfirm>,
                    ].filter(Boolean) as React.ReactElement[]}
                  >
                    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 8 }}>
                      <Text strong style={{ fontSize: 14 }}>{server.name || server.id}</Text>
                      <Badge status={running ? 'success' : 'default'} text={running ? '运行中' : '已停止'} />
                    </div>
                    <div style={{ marginBottom: 4 }}>
                      <Tag color={server.protocol === 'websocket' ? 'blue' : server.protocol === 'http' ? 'purple' : 'green'}>{server.protocol.toUpperCase()}</Tag>
                      {server.mockEnabled && <Tag color="orange">Mock</Tag>}
                      <Text code style={{ fontSize: 11 }}>{server.ip}:{server.port}</Text>
                    </div>
                    <div style={{ color: '#888', fontSize: 11 }}>
                      客户端: {rt?.clientCount ?? 0} | 发: {rt?.sentMessages ?? 0} | 收: {rt?.receivedMessages ?? 0}
                    </div>
                  </Card>
                );
              })}
            </div>
          )}
        </div>

        {/* 右侧：详情面板 */}
        <div style={{ flex: 1, minWidth: 0, overflow: 'auto' }}>
          {selected ? (
            <DetailPanel
              key={selected.id}
              server={selected}
              runtime={runtimes[selected.id]}
              tab={tab}
              onTabChange={setTab}
              onSaveConfig={handleSaveConfig}
              onUpdateMockRules={handleUpdateMockRules}
              onStart={() => startServer(selected.id)}
              onStop={() => stopServer(selected.id)}
              onRestart={() => restartServer(selected.id)}
              messageApi={messageApi}
            />
          ) : (
            <Card variant="outlined" style={{ height: '100%', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
              <Empty description="请在左侧选择一个服务查看详情" />
            </Card>
          )}
        </div>
      </div>

      {/* 新建服务 Modal */}
      <Modal
        title="新增服务"
        open={createOpen}
        onOk={handleCreate}
        onCancel={() => { setCreateOpen(false); createForm.resetFields(); }}
        okText="创建"
        cancelText="取消"
        width={500}
      >
        <Form form={createForm} layout="vertical">
          <Form.Item name="name" label="服务名称" rules={[{ required: true, message: '请输入名称' }]}>
            <Input placeholder="例如：测试服务A" />
          </Form.Item>
          <Form.Item name="protocol" label="协议类型" rules={[{ required: true }]} initialValue="websocket">
            <Select>
              <Option value="websocket">WebSocket</Option>
              <Option value="socket.io">Socket.IO</Option>
              <Option value="http">HTTP</Option>
            </Select>
          </Form.Item>
          <Space style={{ width: '100%' }} size={16}>
            <Form.Item name="ip" label="监听IP" rules={[{ required: true }]} initialValue="0.0.0.0" style={{ flex: 1 }}>
              <Input placeholder="0.0.0.0" />
            </Form.Item>
            <Form.Item name="port" label="监听端口" rules={[{ required: true }]} style={{ width: 150 }}>
              <InputNumber min={1} max={65535} style={{ width: '100%' }} />
            </Form.Item>
          </Space>
          <Form.Item name="description" label="描述">
            <Input placeholder="可选" />
          </Form.Item>
        </Form>
      </Modal>
    </div>
  );
}

// ============== 右侧详情面板 ==============

function DetailPanel({
  server, runtime, tab, onTabChange, onSaveConfig, onUpdateMockRules,
  onStart, onStop, onRestart, messageApi,
}: {
  server: ServerConfig;
  runtime?: ServerRuntime;
  tab: string;
  onTabChange: (t: string) => void;
  onSaveConfig: (vals: Record<string, unknown>) => Promise<void>;
  onUpdateMockRules: (rules: MockRule[]) => Promise<void>;
  onStart: () => void;
  onStop: () => void;
  onRestart: () => void;
  messageApi: ReturnType<typeof message.useMessage>[0];
}) {
  const running = runtime?.status === 'running';

  const tabItems = [
    { key: 'config', label: <span><CloudServerOutlined /> 服务配置</span> },
  ];

  if (server.mockEnabled) {
    tabItems.push({ key: 'mock', label: <span><ThunderboltOutlined /> Mock 规则 ({server.mockRules?.length ?? 0})</span> });
    tabItems.push({ key: 'test', label: <span><ApiOutlined /> 接口测试</span> });
  }

  return (
    <Card variant="outlined" style={{ height: '100%' }}>
      {/* 头部：名称 + 状态 + 操作 */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 12 }}>
        <Space>
          <Title level={4} style={{ margin: 0, fontSize: 18 }}>{server.name || server.id}</Title>
          <Tag color={server.protocol === 'websocket' ? 'blue' : server.protocol === 'http' ? 'purple' : 'green'}>
            {server.protocol.toUpperCase()}
          </Tag>
          {server.mockEnabled && <Tag color="orange">Mock</Tag>}
          <Tag color={running ? 'success' : 'default'}>{running ? '运行中' : '已停止'}</Tag>
          <Text code style={{ fontSize: 12 }}>{server.ip}:{server.port}</Text>
        </Space>
        <Space>
          {!running && <Button size="small" type="primary" icon={<PlayCircleOutlined />} onClick={onStart}>启动</Button>}
          {running && <Button size="small" danger icon={<PauseCircleOutlined />} onClick={onStop}>停止</Button>}
          {running && <Button size="small" icon={<ReloadOutlined />} onClick={onRestart}>重启</Button>}
        </Space>
      </div>

      {/* 运行时统计 */}
      {running && runtime && (
        <div style={{ marginBottom: 12, display: 'flex', gap: 24, fontSize: 12, color: '#888' }}>
          <span>客户端: <Text strong style={{ color: '#52c41a' }}>{runtime.clientCount}</Text></span>
          <span>总连接: {runtime.totalConnections}</span>
          <span>发送: {runtime.sentMessages} ({runtime.sentBytes} bytes)</span>
          <span>接收: {runtime.receivedMessages} ({runtime.receivedBytes} bytes)</span>
        </div>
      )}

      <Tabs activeKey={tab} onChange={onTabChange} items={tabItems.map((t) => ({
        key: t.key,
        label: t.label,
        children: t.key === 'config' ? (
          <ConfigTab server={server} onSave={onSaveConfig} messageApi={messageApi} />
        ) : t.key === 'mock' ? (
          <MockRulesTable
            rules={server.mockRules ?? []}
            onUpdate={onUpdateMockRules}
            messageApi={messageApi}
          />
        ) : t.key === 'test' ? (
          <TestTab server={server} messageApi={messageApi} />
        ) : null,
      }))} />
    </Card>
  );
}

// ============== 服务配置 Tab ==============

function ConfigTab({ server, onSave, messageApi }: {
  server: ServerConfig;
  onSave: (vals: Record<string, unknown>) => Promise<void>;
  messageApi: ReturnType<typeof message.useMessage>[0];
}) {
  const [form] = Form.useForm();
  const protocol = Form.useWatch('protocol', form);
  const mockEnabled = Form.useWatch('mockEnabled', form);

  useEffect(() => {
    form.setFieldsValue({
      name: server.name,
      description: server.description,
      protocol: server.protocol,
      ip: server.ip,
      port: server.port,
      autoStart: server.autoStart,
      logLevel: server.logLevel,
      httpRoutes: server.httpRoutes ?? [],
      mockEnabled: server.mockEnabled ?? false,
      mockDefaultStatusCode: server.mockDefaultStatusCode ?? 200,
      mockDefaultResponseBody: server.mockDefaultResponseBody ?? '{"message":"ok"}',
      mockDefaultDelayMs: server.mockDefaultDelayMs ?? 0,
      mockRules: server.mockRules ?? [],
    });
  }, [server, form]);

  return (
    <Form
      form={form}
      layout="vertical"
      onFinish={(v) => onSave(v)}
    >
      <Space style={{ width: '100%' }} size={16}>
        <Form.Item name="name" label="服务名称" rules={[{ required: true }]} style={{ flex: 1 }}>
          <Input />
        </Form.Item>
        <Form.Item name="protocol" label="协议类型" rules={[{ required: true }]} style={{ width: 150 }}>
          <Select>
            <Option value="websocket">WebSocket</Option>
            <Option value="socket.io">Socket.IO</Option>
            <Option value="http">HTTP</Option>
          </Select>
        </Form.Item>
      </Space>

      <Space style={{ width: '100%' }} size={16}>
        <Form.Item name="ip" label="监听IP" rules={[{ required: true }]} style={{ flex: 1 }}>
          <Input placeholder="0.0.0.0" />
        </Form.Item>
        <Form.Item name="port" label="监听端口" rules={[{ required: true }]} style={{ width: 150 }}>
          <InputNumber min={1} max={65535} style={{ width: '100%' }} />
        </Form.Item>
      </Space>

      <Space style={{ width: '100%' }} size={16}>
        <Form.Item name="autoStart" label="自动启动" valuePropName="checked" style={{ width: 150 }}>
          <Switch checkedChildren="是" unCheckedChildren="否" />
        </Form.Item>
        <Form.Item name="logLevel" label="日志等级" style={{ flex: 1 }}>
          <Select>
            <Option value="DEBUG">DEBUG</Option>
            <Option value="INFO">INFO</Option>
            <Option value="WARN">WARN</Option>
            <Option value="ERROR">ERROR</Option>
          </Select>
        </Form.Item>
      </Space>

      <Form.Item name="description" label="描述">
        <Input placeholder="可选" />
      </Form.Item>

      {/* HTTP 自定义路由 */}
      {protocol === 'http' && (
        <>
          <Divider>HTTP 路由配置</Divider>
          <Text type="secondary" style={{ fontSize: 12, display: 'block', marginBottom: 8 }}>
            自定义接口地址与方法；留空则用内置默认（POST /{`{event}`} 收消息 + GET /stream SSE 推送）。路径支持 <Text code>{'{event}'}</Text> 占位符。
          </Text>
          <Form.List name="httpRoutes">
            {(fields, { add, remove }) => (
              <>
                {fields.map(({ key, name, ...rest }) => (
                  <Space key={key} style={{ display: 'flex', marginBottom: 8 }} align="baseline">
                    <Form.Item {...rest} name={[name, 'method']} rules={[{ required: true }]} style={{ marginBottom: 0 }}>
                      <Select style={{ width: 110 }} placeholder="方法">
                        <Option value="GET">GET</Option>
                        <Option value="POST">POST</Option>
                        <Option value="PUT">PUT</Option>
                        <Option value="DELETE">DELETE</Option>
                        <Option value="PATCH">PATCH</Option>
                      </Select>
                    </Form.Item>
                    <Form.Item {...rest} name={[name, 'path']} rules={[{ required: true }]} style={{ marginBottom: 0 }}>
                      <Input placeholder="/order/{event}" style={{ width: 200 }} />
                    </Form.Item>
                    <Form.Item {...rest} name={[name, 'routeType']} rules={[{ required: true }]} style={{ marginBottom: 0 }}>
                      <Select style={{ width: 110 }} placeholder="类型">
                        <Option value="inbound">收消息</Option>
                        <Option value="stream">SSE推送</Option>
                      </Select>
                    </Form.Item>
                    <Form.Item {...rest} name={[name, 'event']} style={{ marginBottom: 0 }}>
                      <Input placeholder="事件名(可选)" style={{ width: 130 }} />
                    </Form.Item>
                    <Button type="text" danger icon={<DeleteOutlined />} onClick={() => remove(name)} size="small" />
                  </Space>
                ))}
                <Button type="dashed" icon={<PlusOutlined />} onClick={() => add({ method: 'POST', routeType: 'inbound', path: '/' })} block>
                  新增路由
                </Button>
              </>
            )}
          </Form.List>
        </>
      )}

      {/* 统一路由：Mock HTTP */}
      <Divider>统一路由（Mock HTTP + Socket 共端口）</Divider>
      <Form.Item name="mockEnabled" label="启用 Mock HTTP" valuePropName="checked" tooltip="启用后，非 WebSocket 升级的 HTTP 请求将由 Mock 引擎处理；WebSocket 升级请求仍由 Socket 传输层处理">
        <Switch checkedChildren="开" unCheckedChildren="关" />
      </Form.Item>

      {mockEnabled && (
        <>
          <Text type="secondary" style={{ fontSize: 12, display: 'block', marginBottom: 12 }}>
            启用后，该服务端口同时处理 Socket 连接和 HTTP Mock 请求，通过请求类型自动区分。请切换到「Mock 规则」标签配置规则，「接口测试」标签进行测试。
          </Text>
          <Space style={{ width: '100%' }} size={16}>
            <Form.Item name="mockDefaultStatusCode" label="默认状态码" rules={[{ required: true }]} style={{ width: 150 }}>
              <InputNumber min={100} max={599} style={{ width: '100%' }} />
            </Form.Item>
            <Form.Item name="mockDefaultDelayMs" label="默认延迟 (ms)" style={{ width: 150 }}>
              <InputNumber min={0} max={60000} style={{ width: '100%' }} />
            </Form.Item>
          </Space>
          <Form.Item name="mockDefaultResponseBody" label="默认响应体" rules={[{ required: true }]}>
            <JsonEditor rows={4} />
          </Form.Item>
        </>
      )}

      <Form.Item>
        <Button type="primary" htmlType="submit">保存配置</Button>
      </Form.Item>
    </Form>
  );
}

// ============== 接口测试 Tab ==============

function TestTab({ server, messageApi }: {
  server: ServerConfig;
  messageApi: ReturnType<typeof message.useMessage>[0];
}) {
  const [method, setMethod] = useState<HttpMethod>('GET');
  const [path, setPath] = useState('/');
  const [headers, setHeaders] = useState<Array<{ k: string; v: string }>>([{ k: '', v: '' }]);
  const [body, setBody] = useState('');
  const [resp, setResp] = useState<{ status: number; headers: Record<string, string>; body: string; time: number } | null>(null);
  const [sending, setSending] = useState(false);

  const baseUrl = `http://127.0.0.1:${server.port}`;
  const fullUrl = `${baseUrl}${path.startsWith('/') ? path : '/' + path}`;

  const tryPrettyJson = (raw: string): string => {
    try { return JSON.stringify(JSON.parse(raw), null, 2); } catch { return raw; }
  };

  const send = async () => {
    setSending(true);
    setResp(null);
    const t0 = Date.now();
    try {
      const hdrs: Record<string, string> = {};
      headers.forEach((h) => { if (h.k) hdrs[h.k] = h.v; });
      if (body && !['GET', 'HEAD'].includes(method) && !Object.keys(hdrs).some((k) => k.toLowerCase() === 'content-type')) {
        hdrs['Content-Type'] = 'application/json';
      }
      const init: RequestInit = { method, headers: hdrs };
      if (!['GET', 'HEAD'].includes(method) && body) init.body = body;
      const r = await fetch(fullUrl, init);
      const text = await r.text();
      const respHdrs: Record<string, string> = {};
      r.headers.forEach((v, k) => { respHdrs[k] = v; });
      setResp({ status: r.status, headers: respHdrs, body: tryPrettyJson(text), time: Date.now() - t0 });
    } catch (e) {
      messageApi.error(`请求失败：${(e as Error).message}`);
    } finally {
      setSending(false);
    }
  };

  return (
    <div>
      <Space direction="vertical" style={{ width: '100%' }} size={12}>
        <Space wrap>
          <Select value={method} onChange={setMethod} style={{ width: 110 }}>
            {TEST_METHODS.map((m) => <Option key={m} value={m}>{m}</Option>)}
          </Select>
          <Input value={path} onChange={(e) => setPath(e.target.value)} style={{ width: 300 }} placeholder="/users/123" />
          <Button type="primary" onClick={send} loading={sending}>发送</Button>
          <Text type="secondary" style={{ fontSize: 12 }}>目标：{fullUrl}</Text>
        </Space>

        <Divider style={{ margin: '8px 0' }}>请求头</Divider>
        {headers.map((h, i) => (
          <Space key={i} style={{ width: '100%' }}>
            <Input placeholder="Header name" value={h.k} onChange={(e) => setHeaders(headers.map((x, idx) => idx === i ? { ...x, k: e.target.value } : x))} style={{ width: 180 }} />
            <Input placeholder="value" value={h.v} onChange={(e) => setHeaders(headers.map((x, idx) => idx === i ? { ...x, v: e.target.value } : x))} style={{ width: 240 }} />
            <Button danger size="small" icon={<DeleteOutlined />} onClick={() => setHeaders(headers.filter((_, idx) => idx !== i))} />
          </Space>
        ))}
        <Button size="small" onClick={() => setHeaders([...headers, { k: '', v: '' }])}>+ 添加请求头</Button>

        <Divider style={{ margin: '8px 0' }}>请求体</Divider>
        <JsonEditor value={body} onChange={setBody} rows={4} />

        {resp && (
          <>
            <Divider style={{ margin: '8px 0' }}>响应</Divider>
            <Space>
              <Tag color={resp.status >= 200 && resp.status < 300 ? 'green' : 'orange'}>{resp.status}</Tag>
              <Tag>{resp.time}ms</Tag>
              <Text type="secondary" style={{ fontSize: 12 }}>{Object.keys(resp.headers).length} 个响应头</Text>
            </Space>
            <TextArea rows={10} value={resp.body} readOnly style={{ fontFamily: 'monospace' }} />
            <details>
              <summary style={{ cursor: 'pointer', color: '#888' }}>响应头</summary>
              <pre style={{ fontSize: 12, background: '#fafafa', padding: 8, borderRadius: 4 }}>
                {Object.entries(resp.headers).map(([k, v]) => `${k}: ${v}`).join('\n')}
              </pre>
            </details>
          </>
        )}
      </Space>
    </div>
  );
}

/**
 * ServerManagerPage — 服务管理工作台
 *
 * 左列表 + 右详情：扁平侧栏风格，减少卡片嵌套。
 */
import React, { useEffect, useState } from 'react';
import {
  Alert, Button, Form, Input, InputNumber, Modal, Select, Space, Spin, Typography, message, theme,
} from 'antd';
import {
  PauseCircleOutlined, PlayCircleOutlined, PlusOutlined, ReloadOutlined,
} from '@ant-design/icons';
import type { ServerConfig } from '../../types/index.js';
import { useServerStore } from '../../store/useServerStore.js';
import { ServerList, type StatusFilter } from './components/ServerList.js';
import { ServerWorkbench } from './components/ServerWorkbench.js';
import type { WorkbenchSection } from './protocolStyles.js';

const { Title, Text } = Typography;
const { Option } = Select;

export function ServerManagerPage(): React.ReactElement {
  const { token } = theme.useToken();
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
  const [section, setSection] = useState<WorkbenchSection>('overview');
  const [createOpen, setCreateOpen] = useState(false);
  const [createForm] = Form.useForm();
  const [statusFilter, setStatusFilter] = useState<StatusFilter>('all');
  const [selectedKeys, setSelectedKeys] = useState<string[]>([]);

  useEffect(() => {
    fetchServers();
    fetchRuntimes();
  }, []);

  useEffect(() => {
    if (!selectedId && list.length > 0) setSelectedId(list[0].id);
    if (selectedId && !list.some((s) => s.id === selectedId)) {
      setSelectedId(list[0]?.id ?? null);
    }
  }, [list, selectedId]);

  const selected = list.find((s) => s.id === selectedId) || null;

  const counts = {
    all: list.length,
    running: list.filter((s) => runtimes[s.id]?.status === 'running').length,
    stopped: list.filter((s) => runtimes[s.id]?.status !== 'running').length,
  };

  const filteredList = list.filter((s) => {
    if (statusFilter === 'running') return runtimes[s.id]?.status === 'running';
    if (statusFilter === 'stopped') return runtimes[s.id]?.status !== 'running';
    return true;
  });

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
        certPath: null,
        keyPath: null,
        httpRoutes: [],
        mockEnabled: false,
        mockRules: [],
        mockDefaultStatusCode: 200,
        mockDefaultResponseBody: '{"message":"ok"}',
        mockDefaultDelayMs: 0,
      });
      setSelectedId(created.id);
      setSection('basics');
      setCreateOpen(false);
      createForm.resetFields();
      messageApi.success('已创建，可在右侧完善配置');
    } catch (e) {
      messageApi.error('创建失败：' + (e as Error).message);
    }
  };

  const handleSaveBasics = async (vals: Record<string, unknown>) => {
    if (!selected) return;
    try {
      await updateServer(selected.id, vals as Partial<ServerConfig>);
      messageApi.success('基础配置已保存');
    } catch (e) {
      messageApi.error('保存失败：' + (e as Error).message);
    }
  };

  const handleSaveMock = async (vals: Record<string, unknown>) => {
    if (!selected) return;
    try {
      await updateServer(selected.id, vals as Partial<ServerConfig>);
      messageApi.success('Mock 设置已保存');
    } catch (e) {
      messageApi.error('保存失败：' + (e as Error).message);
    }
  };

  const handleUpdateMockRules = async (rules: import('../../types/index.js').MockRule[]) => {
    if (!selected) return;
    await updateServer(selected.id, { mockRules: rules });
  };

  const handleRemove = async (id: string) => {
    await removeServer(id);
    setSelectedKeys((keys) => keys.filter((k) => k !== id));
    if (selectedId === id) setSelectedId(null);
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
      {contextHolder}

      <div
        style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          gap: 8,
          flexWrap: 'wrap',
          marginBottom: 12,
        }}
      >
        <Space size={8}>
          <Title level={4} style={{ margin: 0, fontSize: 16, fontWeight: 600 }}>服务管理</Title>
          {loading && <Spin size="small" />}
        </Space>
        <Space size={8} wrap>
          <Button type="primary" icon={<PlusOutlined />} onClick={() => { createForm.resetFields(); setCreateOpen(true); }}>
            新建
          </Button>
          <Button icon={<PlayCircleOutlined />} onClick={() => startAll()} disabled={list.length === 0}>
            全部启动
          </Button>
          <Button icon={<PauseCircleOutlined />} onClick={() => stopAll()} disabled={list.length === 0}>
            全部停止
          </Button>
          <Button icon={<ReloadOutlined />} onClick={() => restartAll()} disabled={list.length === 0}>
            全部重启
          </Button>
        </Space>
      </div>

      {error && <Alert type="error" message={error} style={{ marginBottom: 12 }} closable />}

      <div
        className="tech-panel"
        style={{
          display: 'flex',
          flex: 1,
          minHeight: 0,
          border: `1px solid ${token.colorBorder}`,
          borderRadius: 10,
          overflow: 'hidden',
          background: token.colorBgContainer,
          boxShadow: `0 0 0 1px ${token.colorBorderSecondary}`,
        }}
      >
        <aside
          className="tech-panel-aside"
          style={{
            width: 280,
            flexShrink: 0,
            display: 'flex',
            flexDirection: 'column',
            minHeight: 0,
            padding: 12,
            borderRight: `1px solid ${token.colorBorder}`,
            background: token.colorFillAlter,
          }}
        >
          <ServerList
            list={filteredList}
            runtimes={runtimes}
            loading={loading}
            selectedId={selectedId}
            selectedKeys={selectedKeys}
            statusFilter={statusFilter}
            counts={counts}
            onFilterChange={setStatusFilter}
            onSelect={(id) => { setSelectedId(id); setSection('overview'); }}
            onSelectionChange={setSelectedKeys}
            onStart={startServer}
            onStop={stopServer}
            onRestart={restartServer}
            onRemove={handleRemove}
            onBatchStart={() => { batchStart(selectedKeys); setSelectedKeys([]); }}
            onBatchStop={() => { batchStop(selectedKeys); setSelectedKeys([]); }}
            onBatchRestart={() => { batchRestart(selectedKeys); setSelectedKeys([]); }}
            onBatchDelete={() => { batchDelete(selectedKeys); setSelectedKeys([]); }}
          />
        </aside>

        <main
          style={{
            flex: 1,
            minWidth: 0,
            overflow: 'hidden',
            display: 'flex',
            flexDirection: 'column',
            padding: 16,
          }}
        >
          {selected ? (
            <ServerWorkbench
              key={selected.id}
              server={selected}
              runtime={runtimes[selected.id]}
              section={section}
              onSectionChange={setSection}
              onSaveBasics={handleSaveBasics}
              onSaveMock={handleSaveMock}
              onUpdateMockRules={handleUpdateMockRules}
              onStart={() => startServer(selected.id)}
              onStop={() => stopServer(selected.id)}
              onRestart={() => restartServer(selected.id)}
              messageApi={messageApi}
            />
          ) : (
            <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
              <Text type="secondary">在左侧选择或新建一个服务</Text>
            </div>
          )}
        </main>
      </div>

      <Modal
        title="新建服务"
        open={createOpen}
        onOk={handleCreate}
        onCancel={() => { setCreateOpen(false); createForm.resetFields(); }}
        okText="创建"
        cancelText="取消"
        width={480}
        destroyOnHidden
      >
        <Form form={createForm} layout="vertical" style={{ marginTop: 8 }}>
          <Form.Item name="name" label="服务名称" rules={[{ required: true, message: '请输入名称' }]}>
            <Input placeholder="例如：联调服务 A" />
          </Form.Item>
          <Form.Item name="protocol" label="协议类型" rules={[{ required: true }]} initialValue="websocket">
            <Select>
              <Option value="websocket">WebSocket</Option>
              <Option value="socket.io">Socket.IO</Option>
              <Option value="http">HTTP</Option>
            </Select>
          </Form.Item>
          <Space style={{ width: '100%' }} size={16}>
            <Form.Item name="ip" label="监听 IP" rules={[{ required: true }]} initialValue="0.0.0.0" style={{ flex: 1 }}>
              <Input placeholder="0.0.0.0" />
            </Form.Item>
            <Form.Item name="port" label="端口" rules={[{ required: true }]} style={{ width: 140 }}>
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

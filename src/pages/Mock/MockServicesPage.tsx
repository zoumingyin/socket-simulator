/**
 * MockServicesPage - Mock 服务管理页面
 * 左侧服务列表，右侧三标签页：服务配置 / 模拟规则 / 接口测试
 *
 * 共享组件（JsonEditor / MockRulesTable 等）提取自 src/components/MockComponents.tsx
 */
import React, { useEffect, useMemo, useState } from 'react';
import {
  Card, Button, Space, Tag, Form, Input, InputNumber, Select, Switch, Modal,
  Tabs, Empty, Popconfirm, message, Typography, Divider,
} from 'antd';
import {
  PlusOutlined, DeleteOutlined,
  PlayCircleOutlined, PauseCircleOutlined, ApiOutlined, ThunderboltOutlined,
} from '@ant-design/icons';
import type {
  MockServiceConfig, HttpMethod,
} from '../../types/index.js';
import { useMockStore } from '../../store/useMockStore.js';
import {
  JsonEditor, MockRulesTable, TEST_METHODS,
} from '../../components/MockComponents.js';

const { Title, Text } = Typography;
const { Option } = Select;
const { TextArea } = Input;

export function MockServicesPage(): React.ReactElement {
  const { list, loading, error, fetchList, add, update, remove, start, stop } = useMockStore();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [tab, setTab] = useState<'config' | 'rules' | 'test'>('config');

  const [messageApi, contextHolder] = message.useMessage();

  useEffect(() => { fetchList(); }, []);

  // 默认选中第一条
  useEffect(() => {
    if (!selectedId && list.length > 0) setSelectedId(list[0].id);
  }, [list, selectedId]);

  const selected = useMemo(
    () => list.find((m) => m.id === selectedId) || null,
    [list, selectedId],
  );

  // ===== 服务配置表单 =====
  const handleSaveConfig = async (values: Partial<MockServiceConfig>) => {
    if (!selected) return;
    const cfg: MockServiceConfig = {
      ...selected,
      name: values.name ?? selected.name,
      description: values.description ?? selected.description,
      basePath: values.basePath ?? selected.basePath,
      customPort: values.customPort,
      defaultStatusCode: values.defaultStatusCode ?? selected.defaultStatusCode,
      defaultResponseBody: values.defaultResponseBody ?? selected.defaultResponseBody,
      defaultDelayMs: values.defaultDelayMs ?? selected.defaultDelayMs,
      enabled: values.enabled ?? selected.enabled,
      updatedAt: new Date().toISOString(),
    };
    try {
      await update(cfg);
      messageApi.success('配置已保存');
    } catch (e) {
      messageApi.error((e as Error).message);
    }
  };

  // ===== 新建服务 =====
  const handleCreate = () => {
    const draft: MockServiceConfig = {
      id: '',
      name: '新 Mock 服务',
      description: '',
      basePath: '/api/example',
      customPort: undefined,
      defaultStatusCode: 200,
      defaultResponseBody: '{"message":"ok"}',
      defaultDelayMs: 0,
      enabled: true,
      rules: [],
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
    };
    Modal.confirm({
      title: '新建 Mock 服务',
      content: (
        <div>
          <p>将创建一个空白的 Mock 服务（basePath=/api/example，无规则）。</p>
          <p>创建后可继续编辑规则。</p>
        </div>
      ),
      onOk: async () => {
        try {
          const created = await add(draft);
          setSelectedId(created.id);
          messageApi.success('已创建');
        } catch (e) {
          messageApi.error((e as Error).message);
        }
      },
    });
  };

  const handleStart = async () => {
    if (!selected) return;
    try {
      await start(selected.id);
      messageApi.success('已启动');
    } catch (e) { messageApi.error((e as Error).message); }
  };
  const handleStop = async () => {
    if (!selected) return;
    try {
      await stop(selected.id);
      messageApi.success('已停止');
    } catch (e) { messageApi.error((e as Error).message); }
  };
  const handleRemove = async () => {
    if (!selected) return;
    try {
      await remove(selected.id);
      setSelectedId(null);
      messageApi.success('已删除');
    } catch (e) { messageApi.error((e as Error).message); }
  };

  return (
    <div style={{ display: 'flex', height: 'calc(100vh - 130px)' }}>
      {contextHolder}

      {/* 左侧：服务列表 */}
      <div style={{ width: 280, borderRight: '1px solid #f0f0f0', padding: '12px 8px', overflowY: 'auto' }}>
        <Space style={{ width: '100%', justifyContent: 'space-between', marginBottom: 12 }}>
          <Title level={5} style={{ margin: 0 }}>服务列表 ({list.length})</Title>
          <Button type="primary" size="small" icon={<PlusOutlined />} onClick={handleCreate}>
            新增
          </Button>
        </Space>
        {error && <Text type="danger" style={{ fontSize: 12 }}>{error}</Text>}
        {list.length === 0 && !loading ? (
          <Empty description="暂无 Mock 服务" />
        ) : (
          <Space direction="vertical" style={{ width: '100%' }} size={8}>
            {list.map((svc) => (
              <Card
                key={svc.id}
                size="small"
                hoverable
                onClick={() => setSelectedId(svc.id)}
                style={{
                  borderColor: svc.id === selectedId ? '#1890ff' : undefined,
                  background: svc.id === selectedId ? '#e6f7ff' : undefined,
                }}
              >
                <div style={{ fontWeight: 500 }}>{svc.name || '(未命名)'}</div>
                <Space size={4} style={{ marginTop: 4, fontSize: 12 }}>
                  <Tag color={svc.enabled ? 'green' : 'default'}>{svc.enabled ? '启用' : '停用'}</Tag>
                  <Tag color="blue">{svc.customPort ? `:${svc.customPort}` : '主端口'}</Tag>
                  <Tag>{svc.rules.length} 条规则</Tag>
                </Space>
                <Text type="secondary" style={{ fontSize: 11, display: 'block', marginTop: 4 }} ellipsis>
                  {svc.basePath}
                </Text>
              </Card>
            ))}
          </Space>
        )}
      </div>

      {/* 右侧：详情 */}
      <div style={{ flex: 1, padding: '16px 24px', overflowY: 'auto' }}>
        {selected ? (
          <>
            <Space style={{ marginBottom: 12 }}>
              <Title level={4} style={{ margin: 0 }}>{selected.name}</Title>
              <Tag color="blue">{selected.basePath}</Tag>
              {selected.enabled
                ? <Button size="small" icon={<PauseCircleOutlined />} onClick={handleStop}>停止</Button>
                : <Button size="small" type="primary" icon={<PlayCircleOutlined />} onClick={handleStart}>启动</Button>}
              <Popconfirm title="确认删除该服务？" onConfirm={handleRemove}>
                <Button size="small" danger icon={<DeleteOutlined />}>删除</Button>
              </Popconfirm>
            </Space>

            <Tabs
              activeKey={tab}
              onChange={(k) => setTab(k as typeof tab)}
              items={[
                {
                  key: 'config',
                  label: <span><ApiOutlined /> 服务配置</span>,
                  children: <ConfigTab cfg={selected} onSave={handleSaveConfig} />,
                },
                {
                  key: 'rules',
                  label: <span><ThunderboltOutlined /> 模拟规则 ({selected.rules.length})</span>,
                  children: <RulesTab cfg={selected} onUpdate={update} messageApi={messageApi} />,
                },
                {
                  key: 'test',
                  label: <span>接口测试</span>,
                  children: <TestTab cfg={selected} messageApi={messageApi} />,
                },
              ]}
            />
          </>
        ) : (
          <Empty description="请在左侧选择或创建一个 Mock 服务" />
        )}
      </div>
    </div>
  );
}

// ============== 服务配置 Tab ==============
function ConfigTab({ cfg, onSave }: {
  cfg: MockServiceConfig;
  onSave: (v: Partial<MockServiceConfig>) => Promise<void>;
}) {
  const [form] = Form.useForm();

  useEffect(() => {
    form.setFieldsValue({
      name: cfg.name,
      description: cfg.description,
      basePath: cfg.basePath,
      customPort: cfg.customPort,
      defaultStatusCode: cfg.defaultStatusCode,
      defaultResponseBody: cfg.defaultResponseBody,
      defaultDelayMs: cfg.defaultDelayMs,
      enabled: cfg.enabled,
    });
  }, [cfg, form]);

  return (
    <Card variant="outlined">
      <Form
        form={form}
        layout="vertical"
        onFinish={(v) => onSave({
          ...v,
          customPort: v.customPort === '' || v.customPort == null ? undefined : Number(v.customPort),
        })}
      >
        <Form.Item name="name" label="服务名称" rules={[{ required: true }]}>
          <Input placeholder="如：用户服务" />
        </Form.Item>

        <Form.Item
          name="basePath"
          label="基础路径 basePath"
          extra="新增/修改后，该路径即作为可访问的 HTTP 接口挂载（不可为 /admin/api 开头）"
          rules={[{ required: true }, { pattern: /^\//, message: '必须以 / 开头' }]}
        >
          <Input placeholder="/api/example" />
        </Form.Item>

        <Form.Item
          name="customPort"
          label="自定义端口（可选）"
          extra="留空则使用主端口；填写后该服务将挂在独立端口 127.0.0.1:<端口> 提供 Mock 接口，与管理窗口隔离（不可与管理窗口端口相同）"
        >
          <InputNumber min={1024} max={65535} style={{ width: '100%' }} placeholder="留空 = 主端口 3080" />
        </Form.Item>

        <Form.Item name="description" label="描述">
          <Input placeholder="可选" />
        </Form.Item>

        <Divider>默认响应（未匹配规则时返回）</Divider>

        <Form.Item name="defaultStatusCode" label="默认状态码" rules={[{ required: true }]}>
          <InputNumber min={100} max={599} style={{ width: 200 }} />
        </Form.Item>

        <Form.Item
          name="defaultResponseBody"
          label="默认响应体"
          rules={[{ required: true }]}
        >
          <JsonEditor rows={5} />
        </Form.Item>

        <Form.Item name="defaultDelayMs" label="默认响应延迟（毫秒）">
          <InputNumber min={0} max={60000} style={{ width: 200 }} />
        </Form.Item>

        <Form.Item name="enabled" label="启用" valuePropName="checked">
          <Switch />
        </Form.Item>

        <Form.Item>
          <Button type="primary" htmlType="submit">保存配置</Button>
        </Form.Item>
      </Form>
    </Card>
  );
}

// ============== 模拟规则 Tab（使用共享 MockRulesTable） ==============
function RulesTab({ cfg, onUpdate, messageApi }: {
  cfg: MockServiceConfig;
  onUpdate: (c: MockServiceConfig) => Promise<MockServiceConfig>;
  messageApi: ReturnType<typeof message.useMessage>[0];
}) {
  return (
    <Card variant="outlined">
      <MockRulesTable
        rules={cfg.rules}
        onUpdate={async (rules) => { await onUpdate({ ...cfg, rules }); }}
        messageApi={messageApi}
      />
    </Card>
  );
}

// ============== 接口测试 Tab ==============
function TestTab({ cfg, messageApi }: { cfg: MockServiceConfig; messageApi: ReturnType<typeof message.useMessage>[0] }) {
  const [method, setMethod] = useState<HttpMethod>('GET');
  const [path, setPath] = useState<string>('/');
  const [headers, setHeaders] = useState<Array<{ k: string; v: string }>>([{ k: '', v: '' }]);
  const [body, setBody] = useState<string>('');
  const [resp, setResp] = useState<{ status: number; headers: Record<string, string>; body: string; time: number } | null>(null);
  const [sending, setSending] = useState(false);

  const baseUrl = cfg.customPort
    ? `http://127.0.0.1:${cfg.customPort}${cfg.basePath}`
    : `http://127.0.0.1:3080${cfg.basePath}`;
  const fullUrl = `${baseUrl}${path.startsWith('/') ? path : '/' + path}`;

  // 尝试 JSON pretty-print
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
      // 有 body 且未手动指定 Content-Type 时自动加 application/json
      if (body && !['GET', 'HEAD'].includes(method) && !Object.keys(hdrs).some((k) => k.toLowerCase() === 'content-type')) {
        hdrs['Content-Type'] = 'application/json';
      }
      const init: RequestInit = { method, headers: hdrs };
      if (!['GET', 'HEAD'].includes(method) && body) init.body = body;
      const r = await fetch(fullUrl, init);
      const text = await r.text();
      const respHdrs: Record<string, string> = {};
      r.headers.forEach((v, k) => { respHdrs[k] = v; });
      // 响应体尝试 JSON 格式化
      const displayBody = tryPrettyJson(text);
      setResp({ status: r.status, headers: respHdrs, body: displayBody, time: Date.now() - t0 });
    } catch (e) {
      messageApi.error(`请求失败：${(e as Error).message}`);
    } finally {
      setSending(false);
    }
  };

  return (
    <Card variant="outlined">
      <Space direction="vertical" style={{ width: '100%' }} size={12}>
        <Space wrap>
          <Select value={method} onChange={setMethod} style={{ width: 110 }}>
            {TEST_METHODS.map((m) => <Option key={m} value={m}>{m}</Option>)}
          </Select>
          <Input
            value={path}
            onChange={(e) => setPath(e.target.value)}
            style={{ width: 320 }}
            placeholder="/users/123"
          />
          <Button type="primary" onClick={send} loading={sending}>发送</Button>
          <Text type="secondary" style={{ fontSize: 12 }}>目标：{fullUrl}</Text>
        </Space>

        <Divider style={{ margin: '8px 0' }}>请求头</Divider>
        {headers.map((h, i) => (
          <Space key={i} style={{ width: '100%' }}>
            <Input
              placeholder="Header name"
              value={h.k}
              onChange={(e) => setHeaders(headers.map((x, idx) => idx === i ? { ...x, k: e.target.value } : x))}
              style={{ width: 180 }}
            />
            <Input
              placeholder="value"
              value={h.v}
              onChange={(e) => setHeaders(headers.map((x, idx) => idx === i ? { ...x, v: e.target.value } : x))}
              style={{ width: 240 }}
            />
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
    </Card>
  );
}

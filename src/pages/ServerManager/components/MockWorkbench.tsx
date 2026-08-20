import React, { useEffect, useState } from 'react';
import {
  Button, Form, Input, InputNumber, Select, Switch, Space, Tag, Typography, theme,
} from 'antd';
import { DeleteOutlined, FileTextOutlined } from '@ant-design/icons';
import type { MessageInstance } from 'antd/es/message/interface';
import type { HttpMethod, MockRule, ServerConfig } from '../../../types/index.js';
import { JsonEditor, MockRulesTable, TEST_METHODS } from '../../../components/MockComponents.js';
import { StatusCodeSelect } from '../../../components/mock/StatusCodeSelect.js';
import { SwaggerImportModal } from '../../../components/mock/SwaggerImportModal.js';

const { Text, Title } = Typography;
const { TextArea } = Input;
const { Option } = Select;

/**
 * MockWorkbench — HTTP Mock 统一工作台（P3-3 抽取）
 *
 * 合并原 `HttpMockSection`（默认响应配置 + 匹配规则）与原 `ProbeSection`
 * （「试跑」HTTP 接口测试面板）为单一可复用组件：
 *   - Config：Mock 开关 + 默认响应（状态码 / 延迟 / 响应体）
 *   - Rules ：匹配规则列表（增删改、启用切换）
 *   - HttpTestPanel：对任意 HTTP 服务发送请求并查看响应（原独立「试跑」Tab，现已内聚）
 *
 * 仅对 http 协议的服务渲染（ServerWorkbench 仅在 http 时展示本 Tab）。
 */
export function MockWorkbench({
  server,
  onSave,
  onUpdateRules,
  messageApi,
}: {
  server: ServerConfig;
  onSave: (vals: Record<string, unknown>) => Promise<void>;
  onUpdateRules: (rules: MockRule[]) => Promise<void>;
  messageApi: MessageInstance;
}): React.ReactElement | null {
  const { token } = theme.useToken();
  const [form] = Form.useForm();
  const [swaggerOpen, setSwaggerOpen] = useState(false);
  const mockEnabled = Form.useWatch('mockEnabled', form);
  const rules = server.mockRules ?? [];
  const enabledCount = rules.filter((r) => r.enabled).length;

  useEffect(() => {
    form.setFieldsValue({
      mockEnabled: server.mockEnabled ?? false,
      mockDefaultStatusCode: server.mockDefaultStatusCode ?? 200,
      mockDefaultResponseBody: server.mockDefaultResponseBody ?? '{"message":"ok"}',
      mockDefaultDelayMs: server.mockDefaultDelayMs ?? 0,
    });
  }, [server, form]);

  // ---- 试跑 / HTTP 接口测试（原 ProbeSection） ----
  const [method, setMethod] = useState<HttpMethod>('GET');
  const [path, setPath] = useState('/');
  const [headers, setHeaders] = useState<Array<{ k: string; v: string }>>([{ k: '', v: '' }]);
  const [body, setBody] = useState('');
  const [resp, setResp] = useState<{ status: number; headers: Record<string, string>; body: string; time: number } | null>(null);
  const [sending, setSending] = useState(false);

  const canProbe = server.protocol === 'http';
  const baseUrl = `http://127.0.0.1:${server.port}`;
  const fullUrl = `${baseUrl}${path.startsWith('/') ? path : `/${path}`}`;

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
      if (body && !['GET', 'HEAD'].includes(method)
        && !Object.keys(hdrs).some((k) => k.toLowerCase() === 'content-type')) {
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

  const panelStyle: React.CSSProperties = {
    border: `1px solid ${token.colorBorderSecondary}`,
    borderRadius: 8,
    background: token.colorFillAlter,
    padding: 12,
    minWidth: 0,
  };

  if (!canProbe) return null;

  return (
    <div
      style={{
        width: '100%',
        maxWidth: '100%',
        minWidth: 0,
        overflowX: 'hidden',
        display: 'flex',
        flexDirection: 'column',
        gap: 12,
      }}
    >
      {/* ============ Config + Rules ============ */}
      <Form
        form={form}
        layout="vertical"
        size="small"
        style={{ maxWidth: '100%' }}
        onFinish={(v) => onSave({
          mockEnabled: v.mockEnabled,
          mockDefaultStatusCode: v.mockDefaultStatusCode,
          mockDefaultResponseBody: v.mockDefaultResponseBody,
          mockDefaultDelayMs: v.mockDefaultDelayMs,
        })}
      >
        {/* 顶栏：开关 + 说明 + 保存 */}
        <div
          style={{
            ...panelStyle,
            padding: '14px 16px',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            gap: 16,
            flexWrap: 'wrap',
            marginBottom: 12,
          }}
        >
          <div style={{ display: 'flex', alignItems: 'center', gap: 16, minWidth: 0, flex: 1 }}>
            <div style={{ display: 'flex', flexDirection: 'column', gap: 6, flexShrink: 0 }}>
              <Text style={{ fontSize: 12, color: token.colorTextSecondary, lineHeight: 1 }}>Mock HTTP</Text>
              <Form.Item name="mockEnabled" valuePropName="checked" style={{ marginBottom: 0 }}>
                <Switch checkedChildren="开" unCheckedChildren="关" />
              </Form.Item>
            </div>
            <div style={{ width: 1, alignSelf: 'stretch', background: token.colorBorderSecondary, flexShrink: 0 }} />
            <Text type="secondary" style={{ fontSize: 12, lineHeight: 1.5, minWidth: 0 }}>
              {mockEnabled
                ? `已启用 · ${enabledCount}/${rules.length} 条规则生效；未命中规则时走下方默认响应`
                : '关闭时仅提供 HTTP 传输默认路由，不走 Mock 规则'}
            </Text>
          </div>
          <Button type="primary" htmlType="submit">保存设置</Button>
        </div>

        {mockEnabled && (
          <div
            style={{
              display: 'grid',
              gridTemplateColumns: 'minmax(0, 280px) minmax(0, 1fr)',
              gap: 12,
              marginBottom: 12,
              alignItems: 'stretch',
            }}
            className="http-mock-defaults-grid"
          >
            <div style={panelStyle}>
              <Title level={5} style={{ margin: '0 0 10px', fontSize: 13, fontWeight: 600 }}>默认响应</Title>
              <Text type="secondary" style={{ fontSize: 12, display: 'block', marginBottom: 10 }}>未命中任何规则时返回</Text>
              <Form.Item name="mockDefaultStatusCode" label="状态码" rules={[{ required: true }]} style={{ marginBottom: 8 }}>
                <StatusCodeSelect allowCustom style={{ width: '100%' }} />
              </Form.Item>
              <Form.Item name="mockDefaultDelayMs" label="延迟 (ms)" style={{ marginBottom: 0 }}>
                <InputNumber min={0} max={60000} style={{ width: '100%' }} />
              </Form.Item>
            </div>
            <div style={{ ...panelStyle, display: 'flex', flexDirection: 'column' }}>
              <Title level={5} style={{ margin: '0 0 10px', fontSize: 13, fontWeight: 600 }}>默认响应体</Title>
              <Form.Item name="mockDefaultResponseBody" rules={[{ required: true }]} style={{ marginBottom: 0, flex: 1 }}>
                <JsonEditor rows={5} />
              </Form.Item>
            </div>
          </div>
        )}
      </Form>

      {mockEnabled ? (
        <div style={{ ...panelStyle, padding: '12px 12px 8px' }}>
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
              gap: 8,
              marginBottom: 4,
              flexWrap: 'wrap',
            }}
          >
            <Space size={8} align="baseline">
              <Title level={5} style={{ margin: 0, fontSize: 13, fontWeight: 600 }}>匹配规则</Title>
              <Text type="secondary" style={{ fontSize: 12 }}>按顺序匹配，首个命中生效</Text>
            </Space>
            <Button size="small" icon={<FileTextOutlined />} onClick={() => setSwaggerOpen(true)}>
              导入 Swagger
            </Button>
          </div>
          <MockRulesTable rules={rules} onUpdate={onUpdateRules} messageApi={messageApi} />
          <SwaggerImportModal
            open={swaggerOpen}
            onCancel={() => setSwaggerOpen(false)}
            onImport={async (imported) => {
              await onUpdateRules([...rules, ...imported]);
            }}
          />
        </div>
      ) : (
        <div style={{ ...panelStyle, textAlign: 'center', padding: '28px 16px' }}>
          <Text type="secondary">开启 Mock 后可配置默认响应与匹配规则，并在下方「接口试跑」中验证。</Text>
        </div>
      )}

      {/* ============ HttpTestPanel（原「试跑」Tab） ============ */}
      <div style={{ ...panelStyle, padding: '12px 12px 12px' }}>
        <Title level={5} style={{ margin: '0 0 4px', fontSize: 13, fontWeight: 600 }}>接口试跑</Title>
        <Text type="secondary" style={{ fontSize: 12, display: 'block', marginBottom: 12 }}>
          向本服务 HTTP 端口发送请求，验证路由 / Mock 规则是否生效
        </Text>
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: 'minmax(0, 1fr) minmax(0, 1fr)',
            gap: 16,
            alignItems: 'start',
          }}
          className="probe-grid"
        >
          <Space direction="vertical" size={12} style={{ width: '100%' }}>
            <Text type="secondary" style={{ fontSize: 12 }}>目标 {fullUrl}</Text>
            <Space wrap style={{ width: '100%' }}>
              <Select value={method} onChange={setMethod} style={{ width: 110 }}>
                {TEST_METHODS.map((m) => <Option key={m} value={m}>{m}</Option>)}
              </Select>
              <Input
                value={path}
                onChange={(e) => setPath(e.target.value)}
                style={{ width: 220, flex: 1, minWidth: 140 }}
                placeholder="/users/123"
              />
              <Button type="primary" onClick={send} loading={sending}>发送</Button>
            </Space>
            <Text strong style={{ fontSize: 13 }}>请求头</Text>
            {headers.map((h, i) => (
              <Space key={i} style={{ width: '100%' }} wrap>
                <Input
                  placeholder="Header"
                  value={h.k}
                  onChange={(e) => setHeaders(headers.map((x, idx) => (idx === i ? { ...x, k: e.target.value } : x)))}
                  style={{ width: 140 }}
                />
                <Input
                  placeholder="value"
                  value={h.v}
                  onChange={(e) => setHeaders(headers.map((x, idx) => (idx === i ? { ...x, v: e.target.value } : x)))}
                  style={{ width: 160 }}
                />
                <Button
                  danger
                  size="small"
                  icon={<DeleteOutlined />}
                  onClick={() => setHeaders(headers.filter((_, idx) => idx !== i))}
                />
              </Space>
            ))}
            <Button size="small" type="dashed" onClick={() => setHeaders([...headers, { k: '', v: '' }])}>
              添加请求头
            </Button>
            <Text strong style={{ fontSize: 13 }}>请求体</Text>
            <JsonEditor value={body} onChange={setBody} rows={6} />
          </Space>

          <div
            style={{
              border: `1px solid ${token.colorBorderSecondary}`,
              borderRadius: 8,
              padding: 12,
              minHeight: 200,
              background: token.colorFillAlter,
            }}
          >
            <Text strong style={{ display: 'block', marginBottom: 8 }}>响应</Text>
            {!resp ? (
              <Text type="secondary">发送请求后在此查看状态码与正文</Text>
            ) : (
              <Space direction="vertical" size={8} style={{ width: '100%' }}>
                <Space>
                  <Tag color={resp.status >= 200 && resp.status < 300 ? 'success' : 'warning'}>{resp.status}</Tag>
                  <Tag>{resp.time} ms</Tag>
                </Space>
                <TextArea
                  rows={14}
                  value={resp.body}
                  readOnly
                  style={{ fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Consolas, monospace', fontSize: 12 }}
                />
                <details>
                  <summary style={{ cursor: 'pointer', color: token.colorTextSecondary, fontSize: 12 }}>响应头</summary>
                  <pre style={{ fontSize: 11, marginTop: 8, whiteSpace: 'pre-wrap' }}>
                    {Object.entries(resp.headers).map(([k, v]) => `${k}: ${v}`).join('\n')}
                  </pre>
                </details>
              </Space>
            )}
          </div>
        </div>
      </div>

      <style>{`
        @media (max-width: 900px) {
          .http-mock-defaults-grid { grid-template-columns: 1fr !important; }
          .probe-grid { grid-template-columns: 1fr !important; }
        }
      `}</style>
    </div>
  );
}

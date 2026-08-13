import React, { useState } from 'react';
import {
  Button, Input, Select, Space, Tag, Typography, theme,
} from 'antd';
import { DeleteOutlined } from '@ant-design/icons';
import type { MessageInstance } from 'antd/es/message/interface';
import type { HttpMethod, ServerConfig } from '../../../../types/index.js';
import { JsonEditor, TEST_METHODS } from '../../../../components/MockComponents.js';

const { Text } = Typography;
const { TextArea } = Input;
const { Option } = Select;

export function ProbeSection({
  server,
  messageApi,
}: {
  server: ServerConfig;
  messageApi: MessageInstance;
}): React.ReactElement | null {
  const { token } = theme.useToken();
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

  if (!canProbe) {
    return null;
  }

  return (
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

      <style>{`
        @media (max-width: 900px) {
          .probe-grid { grid-template-columns: 1fr !important; }
        }
      `}</style>
    </div>
  );
}

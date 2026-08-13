/**
 * MockComponents — Mock HTTP 规则编辑共享组件
 *
 * 供服务管理「HTTP·Mock」区使用：JsonEditor、ConditionEditor、MockRuleModal、MockRulesTable。
 */
import React, { useEffect, useMemo, useState } from 'react';
import {
  Button, Space, Tag, Form, Input, InputNumber, Select, Switch, Modal,
  Table, Divider, Tooltip, Segmented, message, Typography, Popconfirm,
} from 'antd';
import {
  PlusOutlined, DeleteOutlined, CopyOutlined, ClearOutlined, EditOutlined,
} from '@ant-design/icons';
import type { ColumnsType } from 'antd/es/table';
import type {
  MockRule, MockMatchCondition, HttpMethod, MatchKind,
} from '../types/index.js';

const { Text } = Typography;
const { Option } = Select;

// ============== 常量 ==============

export const MOCK_METHODS: HttpMethod[] = ['GET', 'POST', 'PUT', 'DELETE', 'PATCH', 'HEAD', 'OPTIONS', 'ANY'];
export const TEST_METHODS: HttpMethod[] = ['GET', 'POST', 'PUT', 'DELETE', 'PATCH', 'HEAD', 'OPTIONS'];
export const MATCH_KINDS: MatchKind[] = ['exact', 'contains', 'regex', 'exists'];

export const JSON_TEMPLATES: Array<{ key: string; label: string; value: string }> = [
  {
    key: 'success',
    label: '成功响应',
    value: JSON.stringify({ success: true, data: {}, message: 'ok' }, null, 2),
  },
  {
    key: 'error',
    label: '错误响应',
    value: JSON.stringify({ success: false, error: { code: 'ERROR_CODE', message: '错误描述' } }, null, 2),
  },
  {
    key: 'paginated',
    label: '分页列表',
    value: JSON.stringify({ success: true, data: { list: [], total: 0, page: 1, pageSize: 20 } }, null, 2),
  },
  { key: 'array', label: '空数组', value: '[]' },
  { key: 'object', label: '空对象', value: '{}' },
  {
    key: 'user',
    label: '用户示例',
    value: JSON.stringify({ id: 1, name: '张三', email: 'zhangsan@example.com', role: 'admin', createdAt: '2026-01-01T00:00:00Z' }, null, 2),
  },
  {
    key: 'token',
    label: 'Token 响应',
    value: JSON.stringify({ token: 'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...', expiresIn: 3600, refreshToken: 'refresh_token_here' }, null, 2),
  },
];

type JsonValueType = 'string' | 'number' | 'boolean' | 'null';

// ============== JsonEditor ==============

export function JsonEditor({ value, onChange, rows = 6, placeholder }: {
  value?: string;
  onChange?: (v: string) => void;
  rows?: number;
  placeholder?: string;
}) {
  const [mode, setMode] = useState<'text' | 'kv'>('text');
  const [kvPairs, setKvPairs] = useState<Array<{ key: string; value: string; type: JsonValueType }>>([]);
  const [tplKey, setTplKey] = useState<string | undefined>(undefined);

  const current = value ?? '';

  const jsonValid = useMemo(() => {
    if (!current) return true;
    try { JSON.parse(current); return true; } catch { return false; }
  }, [current]);

  const formatJson = () => {
    try {
      onChange?.(JSON.stringify(JSON.parse(current), null, 2));
      message.success('已格式化');
    } catch { message.warning('JSON 无效，无法格式化'); }
  };

  const minifyJson = () => {
    try {
      onChange?.(JSON.stringify(JSON.parse(current)));
      message.success('已压缩');
    } catch { message.warning('JSON 无效，无法压缩'); }
  };

  const copyJson = async () => {
    try {
      await navigator.clipboard.writeText(current);
      message.success('已复制到剪贴板');
    } catch { message.warning('复制失败'); }
  };

  const clearJson = () => { onChange?.(''); };

  const applyTemplate = (v: string) => {
    onChange?.(v);
    message.success('已插入模板');
  };

  const syncKvToText = (pairs: Array<{ key: string; value: string; type: JsonValueType }>) => {
    const obj: Record<string, unknown> = {};
    pairs.forEach((p) => {
      if (!p.key) return;
      switch (p.type) {
        case 'number': obj[p.key] = Number(p.value) || 0; break;
        case 'boolean': obj[p.key] = p.value === 'true'; break;
        case 'null': obj[p.key] = null; break;
        default: obj[p.key] = p.value;
      }
    });
    onChange?.(JSON.stringify(obj, null, 2));
  };

  const switchToKv = () => {
    try {
      const obj = JSON.parse(current || '{}');
      if (typeof obj !== 'object' || obj === null || Array.isArray(obj)) {
        message.warning('仅支持 JSON 对象类型转换为键值编辑');
        return;
      }
      const pairs = Object.entries(obj).map(([k, v]) => {
        if (v === null) return { key: k, value: '', type: 'null' as JsonValueType };
        if (typeof v === 'boolean') return { key: k, value: String(v), type: 'boolean' as JsonValueType };
        if (typeof v === 'number') return { key: k, value: String(v), type: 'number' as JsonValueType };
        return { key: k, value: String(v), type: 'string' as JsonValueType };
      });
      setKvPairs(pairs.length > 0 ? pairs : [{ key: '', value: '', type: 'string' }]);
      setMode('kv');
    } catch {
      message.warning('JSON 无效，无法转换为键值编辑');
    }
  };

  const kvAdd = () => {
    const next = [...kvPairs, { key: '', value: '', type: 'string' as JsonValueType }];
    setKvPairs(next);
    syncKvToText(next);
  };
  const kvRemove = (i: number) => {
    const next = kvPairs.filter((_, idx) => idx !== i);
    setKvPairs(next);
    syncKvToText(next);
  };
  const kvUpdate = (i: number, patch: Partial<{ key: string; value: string; type: JsonValueType }>) => {
    const next = kvPairs.map((p, idx) => idx === i ? { ...p, ...patch } : p);
    setKvPairs(next);
    syncKvToText(next);
  };

  return (
    <div>
      <div style={{ display: 'flex', alignItems: 'center', gap: 4, marginBottom: 6, flexWrap: 'wrap' }}>
        <Segmented
          size="small"
          value={mode}
          onChange={(v) => (v === 'kv' ? switchToKv() : setMode('text'))}
          options={[
            { label: '文本', value: 'text' },
            { label: '键值', value: 'kv' },
          ]}
        />
        {mode === 'text' && (
          <>
            <Select
              size="small"
              placeholder="插入模板"
              style={{ width: 130 }}
              value={tplKey}
              onChange={(v) => { if (v) { applyTemplate(v); setTplKey(undefined); } }}
              allowClear
              options={JSON_TEMPLATES.map((t) => ({ label: t.label, value: t.value }))}
            />
            <Tooltip title="格式化"><Button size="small" type="text" onClick={formatJson}>格式化</Button></Tooltip>
            <Tooltip title="压缩"><Button size="small" type="text" onClick={minifyJson}>压缩</Button></Tooltip>
          </>
        )}
        {mode === 'kv' && <Button size="small" type="text" onClick={() => setMode('text')}>返回文本</Button>}
        <div style={{ marginLeft: 'auto', display: 'flex', gap: 2 }}>
          <Tooltip title="复制"><Button size="small" type="text" icon={<CopyOutlined />} onClick={copyJson} /></Tooltip>
          <Tooltip title="清空"><Button size="small" type="text" icon={<ClearOutlined />} onClick={clearJson} /></Tooltip>
          <Tag color={jsonValid ? 'green' : 'red'} style={{ fontSize: 11, margin: 0 }}>
            {jsonValid ? 'JSON 有效' : 'JSON 无效'}
          </Tag>
        </div>
      </div>

      {mode === 'text' && (
        <Input.TextArea
          rows={rows}
          value={current}
          onChange={(e) => onChange?.(e.target.value)}
          placeholder={placeholder || '{"key":"value"}'}
          style={{ fontFamily: 'monospace', borderColor: jsonValid ? undefined : '#ff4d4f' }}
        />
      )}

      {mode === 'kv' && (
        <div style={{ border: '1px solid #d9d9d9', borderRadius: 6, padding: 8 }}>
          {kvPairs.map((p, i) => (
            <div key={i} style={{ display: 'flex', gap: 4, marginBottom: 4, alignItems: 'center' }}>
              <Input size="small" placeholder="字段名" value={p.key} onChange={(e) => kvUpdate(i, { key: e.target.value })} style={{ width: 130 }} />
              <Select size="small" value={p.type} onChange={(v: JsonValueType) => kvUpdate(i, { type: v })} style={{ width: 90 }}>
                <Select.Option value="string">string</Select.Option>
                <Select.Option value="number">number</Select.Option>
                <Select.Option value="boolean">boolean</Select.Option>
                <Select.Option value="null">null</Select.Option>
              </Select>
              {p.type === 'boolean' ? (
                <Select size="small" value={p.value === 'true' ? 'true' : 'false'} onChange={(v) => kvUpdate(i, { value: v })} style={{ width: 100 }}>
                  <Select.Option value="true">true</Select.Option>
                  <Select.Option value="false">false</Select.Option>
                </Select>
              ) : p.type === 'null' ? (
                <Input size="small" value="null" disabled style={{ flex: 1 }} />
              ) : (
                <Input size="small" placeholder="值" value={p.value} onChange={(e) => kvUpdate(i, { value: e.target.value })} style={{ flex: 1 }} />
              )}
              <Button danger size="small" icon={<DeleteOutlined />} onClick={() => kvRemove(i)} />
            </div>
          ))}
          <Button size="small" type="dashed" icon={<PlusOutlined />} onClick={kvAdd} style={{ marginTop: 4 }}>添加字段</Button>
        </div>
      )}
    </div>
  );
}

// ============== ConditionEditor ==============

export function ConditionEditor({ valueIsHeaderValue, fieldName: fieldNameProp }: { valueIsHeaderValue?: boolean; fieldName?: string }) {
  const form = Form.useFormInstance();
  const fieldName = fieldNameProp || (valueIsHeaderValue ? 'responseHeaders' : 'matchHeaders');
  return <ConditionEditorInner form={form} fieldName={fieldName} />;
}

function ConditionEditorInner({ form, fieldName }: { form: ReturnType<typeof Form.useFormInstance>; fieldName: string }) {
  const [list, setList] = useState<MockMatchCondition[]>([]);

  useEffect(() => {
    const v = form.getFieldValue(fieldName);
    if (Array.isArray(v)) setList(v);
    const onValuesChange = () => {
      const v2 = form.getFieldValue(fieldName);
      if (Array.isArray(v2)) setList(v2);
    };
    const t = setTimeout(onValuesChange, 50);
    return () => clearTimeout(t);
  }, [form, fieldName]);

  const sync = (next: MockMatchCondition[]) => {
    setList(next);
    form.setFieldValue(fieldName, next);
  };

  const add = () => sync([...list, { key: '', value: '', matchKind: 'exact', enabled: true }]);
  const remove = (i: number) => sync(list.filter((_, idx) => idx !== i));
  const update = (i: number, patch: Partial<MockMatchCondition>) => sync(list.map((c, idx) => idx === i ? { ...c, ...patch } : c));

  return (
    <div>
      {list.length === 0 ? (
        <Text type="secondary" style={{ fontSize: 12 }}>无</Text>
      ) : (
        <Table
          size="small"
          rowKey={(_r, i) => String(i)}
          pagination={false}
          dataSource={list}
          columns={[
            { title: 'Key', dataIndex: 'key', width: 140, render: (v, _r, i) => <Input size="small" value={v} onChange={(e) => update(i as number, { key: e.target.value })} /> },
            { title: 'Value', dataIndex: 'value', render: (v, _r, i) => <Input size="small" value={v} onChange={(e) => update(i as number, { value: e.target.value })} /> },
            { title: 'MatchKind', dataIndex: 'matchKind', width: 110, render: (v: MatchKind, _r, i) => (
              <Select size="small" value={v} onChange={(nv) => update(i as number, { matchKind: nv })}>
                {MATCH_KINDS.map((k) => <Option key={k} value={k}>{k}</Option>)}
              </Select>
            ) },
            { title: '启用', dataIndex: 'enabled', width: 60, render: (v: boolean, _r, i) => <Switch size="small" checked={v} onChange={(c) => update(i as number, { enabled: c })} /> },
            { title: '', key: 'del', width: 60, render: (_v, _r, i) => <Button danger size="small" icon={<DeleteOutlined />} onClick={() => remove(i as number)} /> },
          ]}
        />
      )}
      <Button size="small" type="dashed" style={{ marginTop: 8 }} icon={<PlusOutlined />} onClick={add}>添加条件</Button>
    </div>
  );
}

// ============== MockRuleModal - 可复用的规则编辑弹窗 ==============

export function MockRuleModal({
  open,
  editing,
  onOk,
  onCancel,
}: {
  open: boolean;
  editing: MockRule | null;
  onOk: (rule: MockRule) => Promise<void>;
  onCancel: () => void;
}) {
  const [form] = Form.useForm();
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (open) {
      if (editing) {
        form.setFieldsValue(editing);
      } else {
        form.resetFields();
        form.setFieldsValue({
          method: 'GET',
          pathPattern: '/',
          responseStatusCode: 200,
          responseBody: '{"ok":true}',
          responseDelayMs: 0,
          enabled: true,
          matchHeaders: [],
          matchQuery: [],
          responseHeaders: [],
        });
      }
    }
  }, [open, editing, form]);

  const handleOk = async () => {
    try {
      const vals = await form.validateFields();
      const rule: MockRule = {
        ...(editing || {} as MockRule),
        id: editing?.id || `rule_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`,
        ...vals,
        matchBody: vals.matchBody || undefined,
        description: vals.description || undefined,
      };
      setSaving(true);
      await onOk(rule);
      setSaving(false);
    } catch (e) {
      setSaving(false);
      if ((e as Error).message) message.error((e as Error).message);
    }
  };

  return (
    <Modal
      title={editing ? '编辑规则' : '新增规则'}
      open={open}
      onOk={handleOk}
      onCancel={onCancel}
      confirmLoading={saving}
      width={720}
      okText="保存"
      cancelText="取消"
    >
      <Form form={form} layout="vertical">
        <Space style={{ width: '100%' }} size={16}>
          <Form.Item name="name" label="规则名称" style={{ flex: 1, minWidth: 200 }}>
            <Input placeholder="可选，仅展示用" />
          </Form.Item>
          <Form.Item name="method" label="方法" rules={[{ required: true }]}>
            <Select style={{ width: 120 }}>
              {MOCK_METHODS.map((m) => <Option key={m} value={m}>{m}</Option>)}
            </Select>
          </Form.Item>
        </Space>

        <Form.Item
          name="pathPattern"
          label="路径模式"
          extra="精确：/users  前缀：/users/*  参数：/users/:id"
          rules={[{ required: true }]}
        >
          <Input placeholder="/users 或 /users/* 或 /users/:id" />
        </Form.Item>

        <Divider>匹配条件</Divider>
        <Form.Item label="请求头匹配" name="matchHeaders">
          <ConditionEditor />
        </Form.Item>
        <Form.Item label="查询参数匹配" name="matchQuery">
          <ConditionEditor />
        </Form.Item>
        <Form.Item name="matchBody" label="请求体包含子串（可选）">
          <Input placeholder="匹配请求体中是否包含该字符串" />
        </Form.Item>

        <Divider>响应</Divider>
        <Space style={{ width: '100%' }} size={16}>
          <Form.Item name="responseStatusCode" label="状态码" rules={[{ required: true }]}>
            <InputNumber min={100} max={599} style={{ width: 120 }} />
          </Form.Item>
          <Form.Item name="responseDelayMs" label="延迟(ms)">
            <InputNumber min={0} max={60000} style={{ width: 120 }} />
          </Form.Item>
          <Form.Item name="enabled" label="启用" valuePropName="checked">
            <Switch />
          </Form.Item>
        </Space>

        <Form.Item name="responseHeaders" label="响应头">
          <ConditionEditor valueIsHeaderValue />
        </Form.Item>

        <Form.Item name="responseBody" label="响应体" rules={[{ required: true }]}>
          <JsonEditor rows={6} />
        </Form.Item>

        <Form.Item name="description" label="描述">
          <Input placeholder="可选" />
        </Form.Item>
      </Form>
    </Modal>
  );
}

// ============== MockRulesTable - 可复用的规则列表 + 编辑 ==============

export function MockRulesTable({
  rules,
  onUpdate,
  messageApi,
}: {
  rules: MockRule[];
  onUpdate: (rules: MockRule[]) => Promise<void>;
  messageApi: ReturnType<typeof message.useMessage>[0];
}) {
  const [modalOpen, setModalOpen] = useState(false);
  const [editing, setEditing] = useState<MockRule | null>(null);

  const openNew = () => { setEditing(null); setModalOpen(true); };
  const openEdit = (rule: MockRule) => { setEditing(rule); setModalOpen(true); };

  const handleOk = async (rule: MockRule) => {
    const newRules = editing
      ? rules.map((r) => (r.id === rule.id ? rule : r))
      : [...rules, rule];
    await onUpdate(newRules);
    messageApi.success('已保存');
    setModalOpen(false);
  };

  const handleDelete = async (id: string) => {
    await onUpdate(rules.filter((r) => r.id !== id));
    messageApi.success('已删除');
  };

  const handleToggle = async (rule: MockRule, enabled: boolean) => {
    await onUpdate(rules.map((r) => (r.id === rule.id ? { ...r, enabled } : r)));
  };

  const columns: ColumnsType<MockRule> = [
    { title: '方法', dataIndex: 'method', key: 'method', width: 80, render: (v: HttpMethod) => <Tag color="blue">{v}</Tag> },
    { title: '路径', dataIndex: 'pathPattern', key: 'pathPattern', ellipsis: true },
    { title: '状态码', dataIndex: 'responseStatusCode', key: 'responseStatusCode', width: 80 },
    { title: '延迟(ms)', dataIndex: 'responseDelayMs', key: 'responseDelayMs', width: 90 },
    { title: '启用', dataIndex: 'enabled', key: 'enabled', width: 70, render: (v: boolean, r: MockRule) => <Switch size="small" checked={v} onChange={(c) => handleToggle(r, c)} /> },
    {
      title: '操作', key: 'actions', width: 160, render: (_: unknown, r: MockRule) => (
        <Space size={4}>
          <Button size="small" icon={<EditOutlined />} onClick={() => openEdit(r)}>编辑</Button>
          <Popconfirm title="确认删除？" onConfirm={() => handleDelete(r.id)}>
            <Button size="small" danger icon={<DeleteOutlined />}>删除</Button>
          </Popconfirm>
        </Space>
      ),
    },
  ];

  return (
    <div>
      <Space style={{ marginBottom: 12 }}>
        <Button type="primary" icon={<PlusOutlined />} onClick={openNew}>新增规则</Button>
        <Text type="secondary" style={{ fontSize: 12 }}>
          规则按顺序匹配，首个命中规则决定响应；未命中走默认响应
        </Text>
      </Space>
      <Table rowKey="id" columns={columns} dataSource={rules} pagination={{ pageSize: 10 }} size="small" />
      <MockRuleModal open={modalOpen} editing={editing} onOk={handleOk} onCancel={() => setModalOpen(false)} />
    </div>
  );
}

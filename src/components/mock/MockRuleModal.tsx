import { useEffect, useState } from 'react';
import { Form, Input, InputNumber, Select, Switch, Modal, Collapse, Space, Button, message } from 'antd';
import { DeleteOutlined } from '@ant-design/icons';
import { JsonEditor } from './JsonEditor.js';
import { ConditionEditor } from './ConditionEditor.js';
import { MOCK_METHODS } from './constants.js';
import { StatusCodeSelect } from './StatusCodeSelect.js';
import { emptyParam, parseBodyToParams, paramsToBody } from './responseParams.js';
import type { RespParam, RespType } from './responseParams.js';
import type { MockRule } from '../../types/index.js';

const { Option } = Select;

const RESP_TYPE_OPTIONS: Array<{ value: RespType; label: string }> = [
  { value: 'string', label: 'string' },
  { value: 'number', label: 'number' },
  { value: 'boolean', label: 'boolean' },
  { value: 'object', label: 'object {}' },
  { value: 'array', label: 'array []' },
  { value: 'null', label: 'null' },
];

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
  const [advancedKeys, setAdvancedKeys] = useState<string[]>([]);
  const [respKeys, setRespKeys] = useState<string[]>([]);
  const [params, setParams] = useState<RespParam[]>([]);

  useEffect(() => {
    if (open) {
      setAdvancedKeys([]);
      if (editing) {
        form.setFieldsValue(editing);
        const hasAdvanced =
          (editing.matchHeaders?.length ?? 0) > 0
          || (editing.matchQuery?.length ?? 0) > 0
          || !!editing.matchBody
          || (editing.responseHeaders?.length ?? 0) > 0
          || !!editing.description;
        if (hasAdvanced) setAdvancedKeys(['advanced']);
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
        group: vals.group ? String(vals.group).trim() : undefined,
      };
      setSaving(true);
      await onOk(rule);
      setSaving(false);
    } catch (e) {
      setSaving(false);
      if ((e as Error).message) message.error((e as Error).message);
    }
  };

  // ---- 响应参数 ↔ 响应体 双向转换 ----
  const updateParam = (i: number, patch: Partial<RespParam>) =>
    setParams((prev) => prev.map((p, idx) => (idx === i ? { ...p, ...patch } : p)));
  const removeParam = (i: number) =>
    setParams((prev) => prev.filter((_, idx) => idx !== i));

  const handleParseParams = () => {
    const body = form.getFieldValue('responseBody');
    const list = parseBodyToParams(typeof body === 'string' ? body : '');
    if (list.length === 0) {
      message.warning('响应体不是合法 JSON，无法解析');
      return;
    }
    setParams(list);
    message.success(`已从响应体解析 ${list.length} 个参数`);
  };

  const handleGenBody = () => {
    const json = paramsToBody(params);
    form.setFieldValue('responseBody', json);
    message.success('响应体已按参数生成');
  };

  return (
    <Modal
      title={editing ? '编辑规则' : '新增规则'}
      open={open}
      onOk={handleOk}
      onCancel={onCancel}
      confirmLoading={saving}
      width={560}
      centered
      destroyOnHidden
      okText="保存"
      cancelText="取消"
      styles={{ body: { paddingTop: 12, overflow: 'visible' } }}
    >
      <Form form={form} layout="vertical" size="small" requiredMark="optional">
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 108px', columnGap: 10 }}>
          <Form.Item name="name" label="规则名称" style={{ marginBottom: 8 }}>
            <Input placeholder="可选" />
          </Form.Item>
          <Form.Item name="method" label="方法" rules={[{ required: true }]} style={{ marginBottom: 8 }}>
            <Select>
              {MOCK_METHODS.map((m) => <Option key={m} value={m}>{m}</Option>)}
            </Select>
          </Form.Item>
        </div>

        <Form.Item
          name="pathPattern"
          label="路径"
          extra="精确 /users · 前缀 /users/* · 参数 /users/:id"
          rules={[{ required: true }]}
          style={{ marginBottom: 8 }}
        >
          <Input placeholder="/users 或 /users/:id" />
        </Form.Item>

        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 80px', columnGap: 10 }}>
          <Form.Item name="responseStatusCode" label="状态码" rules={[{ required: true }]} style={{ marginBottom: 8 }}>
            <StatusCodeSelect style={{ width: '100%' }} />
          </Form.Item>
          <Form.Item name="responseDelayMs" label="延迟(ms)" style={{ marginBottom: 8 }}>
            <InputNumber min={0} max={60000} style={{ width: '100%' }} />
          </Form.Item>
          <Form.Item name="enabled" label="启用" valuePropName="checked" style={{ marginBottom: 8 }}>
            <Switch />
          </Form.Item>
        </div>

        <Form.Item name="responseBody" label="响应体" rules={[{ required: true }]} style={{ marginBottom: 4 }}>
          <JsonEditor rows={3} />
        </Form.Item>

        <Collapse
          ghost
          size="small"
          activeKey={respKeys}
          onChange={(keys) => setRespKeys(keys as string[])}
          style={{ marginBottom: 8 }}
          items={[{
            key: 'respParams',
            label: '响应参数生成（可选）',
            children: (
              <div>
                <Space size={8} style={{ marginBottom: 8 }} wrap>
                  <Button size="small" onClick={handleParseParams}>从响应体解析</Button>
                  <Button size="small" type="primary" onClick={handleGenBody}>生成响应体</Button>
                  <Button size="small" type="dashed" onClick={() => setParams([...params, emptyParam()])}>
                    添加参数
                  </Button>
                </Space>
                {params.length === 0 ? (
                  <div style={{ fontSize: 12, color: '#888' }}>
                    声明响应字段后点击「生成响应体」；也可先写好响应体再点「从响应体解析」编辑。
                  </div>
                ) : (
                  params.map((p, i) => (
                    <Space key={i} style={{ width: '100%', marginBottom: 6 }} align="baseline">
                      <Input
                        placeholder="字段名（a.b 为嵌套）"
                        value={p.key}
                        onChange={(e) => updateParam(i, { key: e.target.value })}
                        style={{ width: 150 }}
                      />
                      <Select
                        value={p.type}
                        onChange={(v) => updateParam(i, { type: v as RespType })}
                        style={{ width: 104 }}
                        options={RESP_TYPE_OPTIONS}
                      />
                      <Input
                        placeholder={p.type === 'array' ? 'JSON 数组，如 ["a","b"]' : '示例值'}
                        value={p.value}
                        onChange={(e) => updateParam(i, { value: e.target.value })}
                        style={{ flex: 1, minWidth: 120 }}
                      />
                      <Button type="text" danger size="small" icon={<DeleteOutlined />} onClick={() => removeParam(i)} />
                    </Space>
                  ))
                )}
              </div>
            ),
          }]}
        />

        <Collapse
          ghost
          size="small"
          activeKey={advancedKeys}
          onChange={(keys) => setAdvancedKeys(keys as string[])}
          items={[{
            key: 'advanced',
            label: '高级匹配（可选）',
            children: (
              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', columnGap: 10 }}>
                <Form.Item label="请求头" name="matchHeaders" style={{ marginBottom: 8, gridColumn: '1 / -1' }}>
                  <ConditionEditor />
                </Form.Item>
                <Form.Item label="查询参数" name="matchQuery" style={{ marginBottom: 8, gridColumn: '1 / -1' }}>
                  <ConditionEditor />
                </Form.Item>
                <Form.Item name="matchBody" label="请求体包含" style={{ marginBottom: 8 }}>
                  <Input placeholder="可选子串" />
                </Form.Item>
                <Form.Item name="description" label="描述" style={{ marginBottom: 8 }}>
                  <Input placeholder="可选" />
                </Form.Item>
                <Form.Item name="group" label="分组" tooltip="用于规则分组展示（从 Swagger 导入时取 OpenAPI tags）" style={{ marginBottom: 0, gridColumn: '1 / -1' }}>
                  <Input placeholder="可选，如：用户管理" />
                </Form.Item>
                <Form.Item name="responseHeaders" label="响应头" style={{ marginBottom: 0, gridColumn: '1 / -1' }}>
                  <ConditionEditor valueIsHeaderValue />
                </Form.Item>
              </div>
            ),
          }]}
        />
      </Form>
    </Modal>
  );
}

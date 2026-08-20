import { useEffect, useState } from 'react';
import { Form, Input, InputNumber, Select, Switch, Modal, Collapse, message } from 'antd';
import { JsonEditor } from './JsonEditor.js';
import { ConditionEditor } from './ConditionEditor.js';
import { MOCK_METHODS } from './constants.js';
import { StatusCodeSelect } from './StatusCodeSelect.js';
import type { MockRule } from '../../types/index.js';

const { Option } = Select;

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
            <StatusCodeSelect allowCustom style={{ width: '100%' }} />
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

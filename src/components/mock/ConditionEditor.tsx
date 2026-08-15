import { useEffect, useState } from 'react';
import { Form, Table, Button, Select, Switch, Input, Typography } from 'antd';
const { Text } = Typography;
import { DeleteOutlined, PlusOutlined } from '@ant-design/icons';
import type { MockMatchCondition, MatchKind } from '../../types/index.js';
import { MATCH_KINDS } from './constants.js';

const { Option } = Select;

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

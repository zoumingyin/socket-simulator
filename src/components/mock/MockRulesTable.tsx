import { useState } from 'react';
import { Button, Table, Tag, Space, Popconfirm, message, Switch } from 'antd';
import type { ColumnsType } from 'antd/es/table';
import { EditOutlined, DeleteOutlined, PlusOutlined } from '@ant-design/icons';
import type { MockRule, HttpMethod } from '../../types/index.js';
import { MockRuleModal } from './MockRuleModal.js';

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
    { title: '方法', dataIndex: 'method', key: 'method', width: 72, render: (v: HttpMethod) => <Tag color="blue" style={{ margin: 0 }}>{v}</Tag> },
    { title: '路径', dataIndex: 'pathPattern', key: 'pathPattern', ellipsis: true },
    { title: '状态', dataIndex: 'responseStatusCode', key: 'responseStatusCode', width: 56 },
    { title: '延迟', dataIndex: 'responseDelayMs', key: 'responseDelayMs', width: 56, render: (v: number) => `${v}ms` },
    {
      title: '启用',
      dataIndex: 'enabled',
      key: 'enabled',
      width: 52,
      render: (v: boolean, r: MockRule) => (
        <Switch size="small" checked={v} onChange={(c) => handleToggle(r, c)} />
      ),
    },
    {
      title: '',
      key: 'actions',
      width: 72,
      render: (_: unknown, r: MockRule) => (
        <Space size={0}>
          <Button type="text" size="small" icon={<EditOutlined />} onClick={() => openEdit(r)} />
          <Popconfirm title="确认删除？" onConfirm={() => handleDelete(r.id!)}>
            <Button type="text" size="small" danger icon={<DeleteOutlined />} />
          </Popconfirm>
        </Space>
      ),
    },
  ];

  return (
    <div style={{ width: '100%', maxWidth: '100%', minWidth: 0, overflow: 'hidden' }}>
      <div style={{ marginBottom: 10 }}>
        <Button type="primary" icon={<PlusOutlined />} onClick={openNew}>新增规则</Button>
      </div>
      <Table
        rowKey="id"
        columns={columns}
        dataSource={rules}
        pagination={rules.length > 8 ? { pageSize: 8, size: 'small' } : false}
        size="small"
        tableLayout="fixed"
        style={{ width: '100%' }}
        locale={{ emptyText: '暂无规则，点击上方新增' }}
      />
      <MockRuleModal open={modalOpen} editing={editing} onOk={handleOk} onCancel={() => setModalOpen(false)} />
    </div>
  );
}

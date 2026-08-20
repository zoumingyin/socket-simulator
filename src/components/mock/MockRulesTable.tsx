import { useEffect, useMemo, useState } from 'react';
import type { Key } from 'react';
import { Button, Table, Tag, Space, Popconfirm, message, Switch, Typography, Collapse } from 'antd';
import type { ColumnsType } from 'antd/es/table';
import { EditOutlined, DeleteOutlined, PlusOutlined, FolderOutlined } from '@ant-design/icons';
import type { MockRule, HttpMethod } from '../../types/index.js';
import { MockRuleModal } from './MockRuleModal.js';

const { Text } = Typography;

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
  const [selectedKeys, setSelectedKeys] = useState<Key[]>([]);

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

  const handleBatchDelete = async () => {
    if (selectedKeys.length === 0) return;
    const ids = new Set(selectedKeys.map(String));
    await onUpdate(rules.filter((r) => !ids.has(r.id ?? '')));
    setSelectedKeys([]);
    messageApi.success(`已删除 ${ids.size} 条规则`);
  };

  const handleDeleteAll = async () => {
    await onUpdate([]);
    setSelectedKeys([]);
    messageApi.success('已删除全部规则');
  };

  const handleToggle = async (rule: MockRule, enabled: boolean) => {
    await onUpdate(rules.map((r) => (r.id === rule.id ? { ...r, enabled } : r)));
  };

  /** 行选择（跨分组共享选中态） */
  const rowSelection = {
    selectedRowKeys: selectedKeys,
    onChange: (keys: Key[]) => setSelectedKeys(keys),
  };

  /** 按 group（Swagger tags）分组：有分组按组排列，无分组归「未分组」放最后 */
  const grouped = useMemo(() => {
    const groups: Array<{ group: string; list: MockRule[] }> = [];
    const none: MockRule[] = [];
    const idx = new Map<string, number>();
    for (const r of rules) {
      const g = (r.group ?? '').trim();
      if (!g) {
        none.push(r);
        continue;
      }
      const i = idx.get(g);
      if (i === undefined) {
        idx.set(g, groups.length);
        groups.push({ group: g, list: [r] });
      } else {
        groups[i].list.push(r);
      }
    }
    return { groups, none };
  }, [rules]);

  /** 折叠面板展开态：默认全部展开；新增分组自动加入展开 */
  const [openKeys, setOpenKeys] = useState<string[]>([]);
  useEffect(() => {
    setOpenKeys((prev) => {
      const merged = new Set(prev);
      grouped.groups.forEach((g) => merged.add(g.group));
      if (grouped.none.length > 0) merged.add('未分组');
      return Array.from(merged);
    });
  }, [grouped]);

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

  /** 生成折叠面板项（可折叠分组） */
  const renderPanel = (title: string, list: MockRule[]) => ({
    key: title,
    label: (
      <span style={{ display: 'inline-flex', alignItems: 'center', gap: 6 }}>
        <FolderOutlined style={{ fontSize: 12, opacity: 0.6 }} />
        <Text strong style={{ fontSize: 12 }}>{title}</Text>
        <Text type="secondary" style={{ fontSize: 11 }}>{list.length} 条</Text>
      </span>
    ),
    children: (
      <Table
        rowKey="id"
        columns={columns}
        dataSource={list}
        rowSelection={rowSelection}
        pagination={list.length > 8 ? { pageSize: 8, size: 'small' } : false}
        size="small"
        tableLayout="fixed"
        style={{ width: '100%' }}
      />
    ),
  });

  return (
    <div style={{ width: '100%', maxWidth: '100%', minWidth: 0, overflow: 'hidden' }}>
      <div style={{ marginBottom: 10, display: 'flex', gap: 8, alignItems: 'center' }}>
        <Button type="primary" icon={<PlusOutlined />} onClick={openNew}>新增规则</Button>
        <Popconfirm
          title={`确认删除选中的 ${selectedKeys.length} 条规则？`}
          onConfirm={handleBatchDelete}
          disabled={selectedKeys.length === 0}
        >
          <Button
            danger
            icon={<DeleteOutlined />}
            disabled={selectedKeys.length === 0}
          >
            批量删除{selectedKeys.length > 0 ? ` (${selectedKeys.length})` : ''}
          </Button>
        </Popconfirm>
        {rules.length > 0 && (
          <Popconfirm
            title={`确认删除全部 ${rules.length} 条规则？此操作不可撤销`}
            okText="全部删除"
            okButtonProps={{ danger: true }}
            onConfirm={handleDeleteAll}
          >
            <Button danger type="text" icon={<DeleteOutlined />} style={{ marginLeft: 'auto' }}>
              删除全部
            </Button>
          </Popconfirm>
        )}
      </div>
      {rules.length === 0 ? (
        <Table
          rowKey="id"
          columns={columns}
          dataSource={[]}
          pagination={false}
          size="small"
          tableLayout="fixed"
          style={{ width: '100%' }}
          locale={{ emptyText: '暂无规则，点击上方新增' }}
        />
      ) : (
        <Collapse
          ghost
          size="small"
          activeKey={openKeys}
          onChange={(keys) => setOpenKeys(keys as string[])}
          items={[
            ...grouped.groups.map((g) => renderPanel(g.group, g.list)),
            ...(grouped.none.length > 0 ? [renderPanel('未分组', grouped.none)] : []),
          ]}
        />
      )}
      <MockRuleModal open={modalOpen} editing={editing} onOk={handleOk} onCancel={() => setModalOpen(false)} />
    </div>
  );
}

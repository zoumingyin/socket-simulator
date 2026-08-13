import React, { useState } from 'react';
import {
  Button, Checkbox, Dropdown, Empty, Modal, Popconfirm, Segmented, Space, Spin, Typography, theme,
} from 'antd';
import type { MenuProps } from 'antd';
import {
  DeleteOutlined, EllipsisOutlined, PauseCircleOutlined, PlayCircleOutlined, ReloadOutlined,
} from '@ant-design/icons';
import type { ServerConfig, ServerRuntime } from '../../../types/index.js';
import { isHttpService, protocolLabel } from '../protocolStyles.js';

const { Text } = Typography;

export type StatusFilter = 'all' | 'running' | 'stopped';

function addrText(server: ServerConfig): string {
  return server.ip === '0.0.0.0' ? `*:${server.port}` : `${server.ip}:${server.port}`;
}

export function ServerList({
  list,
  runtimes,
  loading,
  selectedId,
  selectedKeys,
  statusFilter,
  counts,
  onFilterChange,
  onSelect,
  onSelectionChange,
  onStart,
  onStop,
  onRestart,
  onRemove,
  onBatchStart,
  onBatchStop,
  onBatchRestart,
  onBatchDelete,
}: {
  list: ServerConfig[];
  runtimes: Record<string, ServerRuntime>;
  loading: boolean;
  selectedId: string | null;
  selectedKeys: string[];
  statusFilter: StatusFilter;
  counts: { all: number; running: number; stopped: number };
  onFilterChange: (f: StatusFilter) => void;
  onSelect: (id: string) => void;
  onSelectionChange: (ids: string[]) => void;
  onStart: (id: string) => void;
  onStop: (id: string) => void;
  onRestart: (id: string) => void;
  onRemove: (id: string) => void;
  onBatchStart: () => void;
  onBatchStop: () => void;
  onBatchRestart: () => void;
  onBatchDelete: () => void;
}): React.ReactElement {
  const { token } = theme.useToken();
  const [hoverId, setHoverId] = useState<string | null>(null);

  const selectedServers = list.filter((s) => selectedKeys.includes(s.id));
  const allRunning = selectedServers.length > 0
    && selectedServers.every((s) => runtimes[s.id]?.status === 'running');
  const allStopped = selectedServers.length > 0
    && selectedServers.every((s) => runtimes[s.id]?.status !== 'running');

  const allChecked = list.length > 0 && list.every((s) => selectedKeys.includes(s.id));
  const someChecked = list.some((s) => selectedKeys.includes(s.id)) && !allChecked;

  const toggleKey = (id: string, checked: boolean) => {
    if (checked) onSelectionChange([...new Set([...selectedKeys, id])]);
    else onSelectionChange(selectedKeys.filter((k) => k !== id));
  };

  const toggleAll = (checked: boolean) => {
    if (checked) onSelectionChange(list.map((s) => s.id));
    else onSelectionChange([]);
  };

  const confirmRemove = (server: ServerConfig) => {
    Modal.confirm({
      title: '确认删除？',
      content: `将删除服务「${server.name || server.id}」`,
      okText: '删除',
      okType: 'danger',
      cancelText: '取消',
      onOk: () => onRemove(server.id),
    });
  };

  const rowMenu = (server: ServerConfig, running: boolean): MenuProps['items'] => [
    !running
      ? { key: 'start', icon: <PlayCircleOutlined />, label: '启动', onClick: () => onStart(server.id) }
      : null,
    running
      ? { key: 'stop', icon: <PauseCircleOutlined />, label: '停止', danger: true, onClick: () => onStop(server.id) }
      : null,
    running
      ? { key: 'restart', icon: <ReloadOutlined />, label: '重启', onClick: () => onRestart(server.id) }
      : null,
    { type: 'divider' },
    {
      key: 'delete',
      icon: <DeleteOutlined />,
      label: '删除',
      danger: true,
      onClick: () => confirmRemove(server),
    },
  ].filter(Boolean) as MenuProps['items'];

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', minHeight: 0 }}>
      <Segmented
        block
        size="small"
        value={statusFilter}
        onChange={(v) => onFilterChange(v as StatusFilter)}
        options={[
          { label: `全部 ${counts.all}`, value: 'all' },
          { label: `运行 ${counts.running}`, value: 'running' },
          { label: `停止 ${counts.stopped}`, value: 'stopped' },
        ]}
        style={{ marginBottom: 10 }}
      />

      {(list.length > 0 || selectedKeys.length > 0) && (
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            gap: 8,
            minHeight: 28,
            marginBottom: 4,
            padding: '0 2px',
          }}
        >
          {list.length > 0 ? (
            <Checkbox
              checked={allChecked}
              indeterminate={someChecked}
              onChange={(e) => toggleAll(e.target.checked)}
            >
              <Text type="secondary" style={{ fontSize: 12 }}>全选</Text>
            </Checkbox>
          ) : <span />}

          {selectedKeys.length > 0 ? (
            <Space size={4}>
              <Text type="secondary" style={{ fontSize: 12 }}>{selectedKeys.length}</Text>
              {allStopped && (
                <Button type="link" size="small" onClick={onBatchStart} style={{ padding: 0, height: 'auto' }}>
                  启动
                </Button>
              )}
              {allRunning && (
                <Button type="link" size="small" danger onClick={onBatchStop} style={{ padding: 0, height: 'auto' }}>
                  停止
                </Button>
              )}
              <Button type="link" size="small" onClick={onBatchRestart} style={{ padding: 0, height: 'auto' }}>
                重启
              </Button>
              <Popconfirm title={`删除选中的 ${selectedKeys.length} 个服务？`} onConfirm={onBatchDelete}>
                <Button type="link" size="small" danger style={{ padding: 0, height: 'auto' }}>
                  删除
                </Button>
              </Popconfirm>
            </Space>
          ) : (
            <Text type="secondary" style={{ fontSize: 12 }}>{list.length} 个服务</Text>
          )}
        </div>
      )}

      <div style={{ flex: 1, overflow: 'auto', minHeight: 0 }}>
        {loading && list.length === 0 ? (
          <div style={{ padding: 32, textAlign: 'center' }}><Spin /></div>
        ) : list.length === 0 ? (
          <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="暂无服务" style={{ marginTop: 48 }} />
        ) : (
          list.map((server) => {
            const rt = runtimes[server.id];
            const running = rt?.status === 'running';
            const active = selectedId === server.id;
            const hovering = hoverId === server.id;
            const clients = rt?.clientCount ?? 0;
            const showMock = isHttpService(server.protocol) && server.mockEnabled;
            const meta = [
              protocolLabel(server.protocol),
              showMock ? 'Mock' : null,
              addrText(server),
              running ? `${clients} 连接` : null,
            ].filter(Boolean).join(' · ');

            return (
              <div
                key={server.id}
                role="button"
                tabIndex={0}
                onClick={() => onSelect(server.id)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' || e.key === ' ') {
                    e.preventDefault();
                    onSelect(server.id);
                  }
                }}
                onMouseEnter={() => setHoverId(server.id)}
                onMouseLeave={() => setHoverId(null)}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: 8,
                  padding: '10px 10px',
                  marginBottom: 6,
                  cursor: 'pointer',
                  borderRadius: 8,
                  border: `1px solid ${
                    active
                      ? token.colorPrimary
                      : hovering
                        ? token.colorBorder
                        : token.colorBorderSecondary
                  }`,
                  background: active
                    ? token.colorPrimaryBg
                    : hovering
                      ? token.colorBgContainer
                      : token.colorBgContainer,
                  boxShadow: active
                    ? `inset 3px 0 0 ${token.colorPrimary}`
                    : running
                      ? `inset 3px 0 0 ${token.colorSuccess}`
                      : `inset 3px 0 0 ${token.colorBorderSecondary}`,
                  transition: 'border-color 0.15s, background 0.15s, box-shadow 0.15s',
                }}
              >
                <Checkbox
                  checked={selectedKeys.includes(server.id)}
                  onClick={(e) => e.stopPropagation()}
                  onChange={(e) => toggleKey(server.id, e.target.checked)}
                />

                <span
                  aria-hidden
                  style={{
                    width: 8,
                    height: 8,
                    borderRadius: '50%',
                    flexShrink: 0,
                    background: running ? token.colorSuccess : token.colorTextQuaternary,
                  }}
                />

                <div style={{ flex: 1, minWidth: 0 }}>
                  <Text
                    strong={active}
                    ellipsis
                    style={{
                      display: 'block',
                      fontSize: 13,
                      lineHeight: '20px',
                      color: token.colorText,
                    }}
                  >
                    {server.name || server.id}
                  </Text>
                  <Text
                    ellipsis
                    type="secondary"
                    title={meta}
                    style={{ display: 'block', fontSize: 12, lineHeight: '18px' }}
                  >
                    {meta}
                  </Text>
                </div>

                <div
                  onClick={(e) => e.stopPropagation()}
                  style={{
                    opacity: hovering || active ? 1 : 0,
                    transition: 'opacity 0.12s',
                    flexShrink: 0,
                  }}
                >
                  <Dropdown menu={{ items: rowMenu(server, running) }} trigger={['click']}>
                    <Button type="text" size="small" icon={<EllipsisOutlined />} />
                  </Dropdown>
                </div>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
}

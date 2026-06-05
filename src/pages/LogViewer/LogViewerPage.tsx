/**
 * LogViewerPage - 日志查看器页面
 * 支持自动滚动、关键字搜索、按服务/事件/客户端过滤、导出/清空日志
 */
import React, { useEffect, useState } from 'react';
import {
  Card, Input, Select, Space, Button, Switch, Table, Tag, Typography, Popover,
  message,
} from 'antd';
import { SearchOutlined, ExportOutlined, ClearOutlined, ReloadOutlined } from '@ant-design/icons';
import type { LogEntry, LogLevel } from '../../types/index.js';
import { useLogStore } from '../../store/useLogStore.js';

const { Title, Text } = Typography;
const { Option } = Select;

export function LogViewerPage(): React.ReactElement {
  const [messageApi, contextHolder] = message.useMessage();
  const { entries, filter, autoScroll, loading, error, fetchLogs, setFilter, toggleAutoScroll, clearLogs } = useLogStore();
  const [exportPath, setExportPath] = useState('');
  const [exporting, setExporting] = useState(false);

  useEffect(() => { fetchLogs(); }, []);

  // 轮询日志（每 3 秒）
  useEffect(() => {
    const timer = setInterval(() => fetchLogs(), 3000);
    return () => clearInterval(timer);
  }, []);

  const handleExport = async () => {
    if (!exportPath) { messageApi.warning('请先设置导出路径'); return; }
    setExporting(true);
    try {
      await new Promise(r => setTimeout(r, 500));
      const data = JSON.stringify(entries, null, 2);
      const blob = new Blob([data], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `socket-logs-${new Date().toISOString().split('T')[0]}.json`;
      a.click();
      URL.revokeObjectURL(url);
      messageApi.success('导出成功');
    } catch (e) {
      messageApi.error('导出失败: ' + (e as Error).message);
    } finally {
      setExporting(false);
    }
  };

  const handleClear = () => {
    clearLogs();
    messageApi.success('日志已清空');
  };

  const levelColors: Record<LogLevel, string> = { DEBUG: 'blue', INFO: 'green', WARN: 'orange', ERROR: 'red' };

  const columns = [
    { title: '时间', dataIndex: 'timestamp', key: 'timestamp', align: 'center' as const,
      render: (v: string) => <Text type="secondary">{new Date(v).toLocaleString()}</Text>,
    },
    { title: '等级', dataIndex: 'level', key: 'level', align: 'center' as const,
      render: (v: LogLevel) => <Tag color={levelColors[v]}>{v}</Tag>,
    },
    { title: '事件', dataIndex: 'event', key: 'event', align: 'center' as const, ellipsis: true },
    { title: '服务ID', dataIndex: 'serverId', key: 'serverId', align: 'center' as const, ellipsis: true,
      render: (v?: string) => v ? <Tag>{v}</Tag> : '-',
    },
    { title: '客户端ID', dataIndex: 'clientId', key: 'clientId', align: 'center' as const, ellipsis: true,
      render: (v?: string) => v ? <Text code>{v}</Text> : '-',
    },
    { title: '推送事件', key: 'pushEvent', align: 'center' as const, ellipsis: true,
      render: (_: unknown, record: LogEntry) =>
        record.metadata?.event ? <Tag color="blue">{String(record.metadata.event)}</Tag> : '-',
    },
    { title: '目标类型', key: 'targetType', align: 'center' as const, ellipsis: true,
      render: (_: unknown, record: LogEntry) =>
        record.metadata?.targetType ? <Tag color="green">{String(record.metadata.targetType)}</Tag> : '-',
    },
    { title: '目标ID', key: 'targetId', align: 'center' as const, ellipsis: true,
      render: (_: unknown, record: LogEntry) =>
        record.metadata?.targetId ? <Text code style={{ fontSize: 12 }}>{String(record.metadata.targetId)}</Text> : '-',
    },
    { title: '消息', dataIndex: 'message', key: 'message', align: 'left' as const,
      ellipsis: false,
      render: (text: string) => {
        const popoverContent = (
          <div style={{ maxHeight: 400, overflow: 'auto', whiteSpace: 'pre-wrap', wordBreak: 'break-all', maxWidth: 600 }}>
            {text}
          </div>
        );
        return (
          <Popover content={popoverContent} title="完整消息" trigger="hover" placement="topLeft" mouseEnterDelay={0.3}>
            <div style={{ textAlign: 'left', maxHeight: 60, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', cursor: 'pointer' }}>
              {text}
            </div>
          </Popover>
        );
      },
    },
  ];

  return (
    <div>
      {contextHolder}
      <Title level={4} style={{ marginBottom: 16, fontSize: 18 }}>日志查看器</Title>

      <Card variant="outlined" style={{ marginBottom: 24 }}>
        <Space direction="vertical" style={{ width: '100%' }} size={16}>
          <Space wrap>
            <Input
              placeholder="关键字搜索"
              prefix={<SearchOutlined />}
              value={filter.keyword ?? ''}
              onChange={e => setFilter({ keyword: e.target.value || undefined })}
              style={{ width: 220 }}
              allowClear
            />
            <Select
              placeholder="日志等级"
              value={filter.level}
              onChange={v => setFilter({ level: v })}
              style={{ width: 130 }}
              allowClear
            >
              <Option value="DEBUG">DEBUG</Option>
              <Option value="INFO">INFO</Option>
              <Option value="WARN">WARN</Option>
              <Option value="ERROR">ERROR</Option>
            </Select>
            <Input
              placeholder="服务ID过滤"
              value={filter.serverId ?? ''}
              onChange={e => setFilter({ serverId: e.target.value || undefined })}
              style={{ width: 150 }}
              allowClear
            />
            <Button icon={<ReloadOutlined />} onClick={() => fetchLogs()}>刷新</Button>
          </Space>

          <Space>
            <Switch checked={autoScroll} onChange={toggleAutoScroll} /> 自动滚动
            <Button icon={<ExportOutlined />} loading={exporting} onClick={handleExport}>导出日志</Button>
            <Button danger icon={<ClearOutlined />} onClick={handleClear}>清空日志</Button>
          </Space>
        </Space>
      </Card>

      <Card variant="outlined" style={{ flex: 1, display: 'flex', flexDirection: 'column' }}>
        {error && <Text type="danger">{error}</Text>}
        <Table
          bordered
          rowKey="id"
          columns={columns}
          dataSource={entries.slice(-500)}
          loading={loading}
          pagination={{ pageSize: 50, showSizeChanger: true, showTotal: t => `共 ${t} 条` }}
          size="small"
          scroll={{ x: 'max-content', y: 'calc(100vh - 380px)' }}
          style={{ flex: 1 }}
        />
      </Card>
    </div>
  );
}

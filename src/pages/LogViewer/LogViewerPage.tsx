/**
 * LogViewerPage - 日志查看器页面
 * 支持自动滚动、关键字搜索、按服务/事件/客户端过滤、导出/清空日志
 * 日志数据通过 WebSocket 实时推送，无轮询
 * 选中日志后在表格下方独立区域展示详细内容
 */
import React, { useEffect, useState } from 'react';
import { Button, Card, Modal, Table, Typography, message } from 'antd';
import { HistoryOutlined } from '@ant-design/icons';
import type { LogEntry } from '../../types/index.js';
import { useLogStore } from '../../store/useLogStore.js';
import { logColumns } from './logColumns.js';
import { LogDetailPanel } from './LogDetailPanel.js';
import { LogToolbar } from './LogToolbar.js';

const { Title, Text } = Typography;

export function LogViewerPage(): React.ReactElement {
  const [messageApi, contextHolder] = message.useMessage();
  const {
    entries,
    historyEntries,
    historyTotal,
    historyLoading,
    filter,
    autoScroll,
    loading,
    error,
    fetchLogs,
    fetchHistory,
    setFilter,
    toggleAutoScroll,
    exportLogs,
    clearLogs,
  } = useLogStore();
  const [exporting, setExporting] = useState(false);
  const [selectedLog, setSelectedLog] = useState<LogEntry | null>(null);

  useEffect(() => {
    fetchLogs();
  }, []);

  /* ---- 操作 ---- */

  const handleExport = async () => {
    setExporting(true);
    try {
      await exportLogs(filter);
      messageApi.success('导出成功');
    } catch (e) {
      messageApi.error('导出失败: ' + (e as Error).message);
    } finally {
      setExporting(false);
    }
  };

  const handleClear = () => {
    Modal.confirm({
      title: '清空日志',
      content: '将清空内存中的日志记录（磁盘文件与历史持久化日志不受影响），确定继续？',
      okText: '清空',
      okButtonProps: { danger: true },
      cancelText: '取消',
      onOk: async () => {
        await clearLogs();
        setSelectedLog(null);
        messageApi.success('日志已清空');
      },
    });
  };

  const handleRowClick = (record: LogEntry) => {
    setSelectedLog((prev) => (prev?.id === record.id ? null : record));
  };

  /* ---- 渲染 ---- */

  // 历史持久化日志（早）+ 实时流日志（晚）合并展示
  const mergedEntries = [...historyEntries, ...entries];

  return (
    <div>
      {contextHolder}
      <div
        style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          marginBottom: 16,
        }}
      >
        <Title level={4} style={{ margin: 0, fontSize: 18 }}>
          日志查看器
        </Title>
        <Button
          icon={<HistoryOutlined />}
          loading={historyLoading}
          onClick={() => fetchHistory()}
        >
          加载历史{historyTotal > 0 ? `（共 ${historyTotal} 条）` : ''}
        </Button>
      </div>

      <LogToolbar
        filter={filter}
        setFilter={setFilter}
        autoScroll={autoScroll}
        toggleAutoScroll={toggleAutoScroll}
        exporting={exporting}
        onRefresh={fetchLogs}
        onExport={handleExport}
        onClear={handleClear}
      />

      {/* 日志表格 + 下方详情面板 */}
      <Card variant="outlined">
        {error && <Text type="danger">{error}</Text>}
        <Table
          bordered
          rowKey="id"
          columns={logColumns}
          dataSource={mergedEntries.slice(-1000)}
          loading={loading || historyLoading}
          pagination={{
            pageSize: 50,
            showSizeChanger: true,
            showTotal: (t) => `共 ${t} 条`,
          }}
          size="small"
          scroll={{ y: 'calc(100vh - 420px)' }}
          rowClassName={(record) =>
            selectedLog?.id === record.id ? 'ant-table-row-selected' : ''
          }
          onRow={(record) => ({
            onClick: () => handleRowClick(record),
            style: { cursor: 'pointer' },
          })}
        />

        {/* 独立详情面板：表格下方 */}
        <LogDetailPanel selectedLog={selectedLog} onClose={() => setSelectedLog(null)} />
      </Card>
    </div>
  );
}

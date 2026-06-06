/**
 * LogViewerPage - 日志查看器页面
 * 支持自动滚动、关键字搜索、按服务/事件/客户端过滤、导出/清空日志
 * 日志数据通过 WebSocket 实时推送，无轮询
 * 选中日志后在表格下方独立区域展示详细内容
 */
import React, { useEffect, useState } from "react";
import {
  Card,
  Input,
  Select,
  Space,
  Button,
  Switch,
  Table,
  Tag,
  Typography,
  message,
} from "antd";
import {
  SearchOutlined,
  ExportOutlined,
  ClearOutlined,
  ReloadOutlined,
  CloseOutlined,
} from "@ant-design/icons";
import type { LogEntry, LogLevel } from "../../types/index.js";
import { useLogStore } from "../../store/useLogStore.js";

const { Title, Text } = Typography;
const { Option } = Select;

/** 表格自适应列宽配置：固定宽度 + 长文本省略 + 消息列换行 */
const COL_WIDTHS = {
  time: 152,
  level: 64,
  event: 110,
  serverId: 96,
  clientId: 106,
  pushEvent: 96,
  targetType: 86,
  targetId: 86,
};

const levelColors: Record<LogLevel, string> = {
  DEBUG: "blue",
  INFO: "green",
  WARN: "orange",
  ERROR: "red",
};

export function LogViewerPage(): React.ReactElement {
  const [messageApi, contextHolder] = message.useMessage();
  const {
    entries,
    filter,
    autoScroll,
    loading,
    error,
    fetchLogs,
    setFilter,
    toggleAutoScroll,
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
      await new Promise((r) => setTimeout(r, 500));
      const data = JSON.stringify(entries, null, 2);
      const blob = new Blob([data], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `socket-logs-${new Date().toISOString().split("T")[0]}.json`;
      a.click();
      URL.revokeObjectURL(url);
      messageApi.success("导出成功");
    } catch (e) {
      messageApi.error("导出失败: " + (e as Error).message);
    } finally {
      setExporting(false);
    }
  };

  const handleClear = () => {
    clearLogs();
    setSelectedLog(null);
    messageApi.success("日志已清空");
  };

  const handleRowClick = (record: LogEntry) => {
    setSelectedLog((prev) => (prev?.id === record.id ? null : record));
  };

  /* ---- 列定义 ---- */
  const columns = [
    {
      title: "时间",
      dataIndex: "timestamp",
      key: "timestamp",
      align: "center" as const,
      width: COL_WIDTHS.time,
      render: (v: string) => (
        <Text type="secondary" style={{ fontSize: 12 }}>
          {new Date(v).toLocaleString()}
        </Text>
      ),
    },
    {
      title: "等级",
      dataIndex: "level",
      key: "level",
      align: "center" as const,
      width: COL_WIDTHS.level,
      render: (v: LogLevel) => <Tag color={levelColors[v]}>{v}</Tag>,
    },
    {
      title: "事件",
      dataIndex: "event",
      key: "event",
      align: "center" as const,
      width: COL_WIDTHS.event,
      ellipsis: true,
    },
    {
      title: "服务ID",
      dataIndex: "serverId",
      key: "serverId",
      align: "center" as const,
      width: COL_WIDTHS.serverId,
      ellipsis: true,
      render: (v?: string) =>
        v ? (
          <Tag
            style={{
              maxWidth: "100%",
              overflow: "hidden",
              textOverflow: "ellipsis",
            }}
          >
            {v}
          </Tag>
        ) : (
          "-"
        ),
    },
    {
      title: "客户端ID",
      dataIndex: "clientId",
      key: "clientId",
      align: "center" as const,
      width: COL_WIDTHS.clientId,
      ellipsis: true,
      render: (v?: string) =>
        v ? (
          <Text code style={{ fontSize: 12 }}>
            {v}
          </Text>
        ) : (
          "-"
        ),
    },
    {
      title: "推送事件",
      key: "pushEvent",
      align: "center" as const,
      width: COL_WIDTHS.pushEvent,
      ellipsis: true,
      render: (_: unknown, record: LogEntry) =>
        record.metadata?.event ? (
          <Tag color="blue">{String(record.metadata.event)}</Tag>
        ) : (
          "-"
        ),
    },
    {
      title: "目标类型",
      key: "targetType",
      align: "center" as const,
      width: COL_WIDTHS.targetType,
      ellipsis: true,
      render: (_: unknown, record: LogEntry) =>
        record.metadata?.targetType ? (
          <Tag color="green">{String(record.metadata.targetType)}</Tag>
        ) : (
          "-"
        ),
    },
    {
      title: "目标ID",
      key: "targetId",
      align: "center" as const,
      width: COL_WIDTHS.targetId,
      ellipsis: true,
      render: (_: unknown, record: LogEntry) =>
        record.metadata?.targetId ? (
          <Text code style={{ fontSize: 12 }}>
            {String(record.metadata.targetId)}
          </Text>
        ) : (
          "-"
        ),
    },
    {
      title: "消息",
      dataIndex: "message",
      key: "message",
      align: "left" as const,
      render: (text: string) => (
        <div
          style={{
            maxHeight: 48,
            overflow: "hidden",
            wordBreak: "break-all",
            whiteSpace: "pre-wrap",
            lineHeight: "20px",
            fontSize: 12,
            cursor: "pointer",
          }}
        >
          {text}
        </div>
      ),
    },
  ];

  /* ---- 详情面板 ---- */

  /** 格式化 metadata，单独解析 content 字段避免转义符 */
  const formatMetadata = (
    metadata: Record<string, unknown> | undefined
  ): string => {
    if (!metadata) return "{}";
    try {
      const meta = JSON.parse(JSON.stringify(metadata));
      if (meta.content && typeof meta.content === "string") {
        try {
          meta.content = JSON.parse(meta.content);
        } catch {}
      }
      return JSON.stringify(meta, null, 2);
    } catch {
      return JSON.stringify(metadata, null, 2);
    }
  };

  const renderDetailPanel = () => {
    if (!selectedLog) return null;
    const record = selectedLog;
    const hasMetadata =
      record.metadata && Object.keys(record.metadata).length > 0;

    return (
      <Card
        variant="outlined"
        style={{ marginTop: 12, backgroundColor: "#fafafa" }}
        title={
          <Space>
            <Tag color={levelColors[record.level]}>{record.level}</Tag>
            <Text strong>{record.event}</Text>
          </Space>
        }
        extra={
          <Button
            type="text"
            size="small"
            icon={<CloseOutlined />}
            onClick={() => setSelectedLog(null)}
          />
        }
      >
        <Space direction="vertical" style={{ width: "100%" }} size={16}>
          {/* 基本信息 */}
          <div>
            <Text
              type="secondary"
              style={{ display: "block", marginBottom: 8 }}
            >
              基本信息
            </Text>
            <Space wrap size={[16, 8]}>
              <div>
                <Text type="secondary">时间：</Text>
                <Text style={{ fontSize: 13 }}>
                  {new Date(record.timestamp).toLocaleString()}
                </Text>
              </div>
              <div>
                <Text type="secondary">服务ID：</Text>
                {record.serverId ? (
                  <Tag>{record.serverId}</Tag>
                ) : (
                  <Text type="secondary">-</Text>
                )}
              </div>
              <div>
                <Text type="secondary">客户端ID：</Text>
                {record.clientId ? (
                  <Text code>{record.clientId}</Text>
                ) : (
                  <Text type="secondary">-</Text>
                )}
              </div>
            </Space>
          </div>

          {/* 推送信息 */}
          {hasMetadata && (
            <div>
              <Text
                type="secondary"
                style={{ display: "block", marginBottom: 8 }}
              >
                推送信息
              </Text>
              <Space wrap size={[16, 8]}>
                {record.metadata?.event ? (
                  <div>
                    <Text type="secondary">推送事件：</Text>
                    <Tag color="blue">{String(record.metadata.event)}</Tag>
                  </div>
                ) : null}
                {record.metadata?.targetType ? (
                  <div>
                    <Text type="secondary">目标类型：</Text>
                    <Tag color="green">
                      {String(record.metadata.targetType)}
                    </Tag>
                  </div>
                ) : null}
                {record.metadata?.targetId ? (
                  <div>
                    <Text type="secondary">目标ID：</Text>
                    <Text code style={{ fontSize: 12 }}>
                      {String(record.metadata.targetId)}
                    </Text>
                  </div>
                ) : null}
              </Space>
            </div>
          )}

          {/* 消息内容 */}
          <div>
            <Text
              type="secondary"
              style={{ display: "block", marginBottom: 8 }}
            >
              消息内容
            </Text>
            <pre
              style={{
                backgroundColor: "#fff",
                border: "1px solid #d9d9d9",
                borderRadius: 6,
                padding: 12,
                maxHeight: 400,
                overflow: "auto",
                whiteSpace: "pre-wrap",
                wordBreak: "break-all",
                fontFamily:
                  "'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, monospace",
                fontSize: 13,
                lineHeight: 1.6,
                margin: 0,
              }}
            >
              {record.message || <Text type="secondary">（无消息内容）</Text>}
            </pre>
          </div>

          {/* 原始 metadata —— 单独解析 content 字段避免转义符 */}
          {hasMetadata && (
            <div>
              <Text
                type="secondary"
                style={{ display: "block", marginBottom: 8 }}
              >
                原始 Metadata
              </Text>
              <div
                style={{
                  backgroundColor: "#fff",
                  border: "1px solid #d9d9d9",
                  borderRadius: 6,
                  padding: 12,
                  maxHeight: 300,
                  overflow: "auto",
                  whiteSpace: "pre-wrap",
                  wordBreak: "break-all",
                  fontFamily:
                    "'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, monospace",
                  fontSize: 12,
                  lineHeight: 1.5,
                  color: "#595959",
                }}
              >
                {formatMetadata(record.metadata)}
              </div>
            </div>
          )}
        </Space>
      </Card>
    );
  };

  /* ---- 渲染 ---- */
  return (
    <div>
      {contextHolder}
      <Title level={4} style={{ marginBottom: 16, fontSize: 18 }}>
        日志查看器
      </Title>

      {/* 筛选栏 */}
      <Card variant="outlined" style={{ marginBottom: 24 }}>
        <Space direction="vertical" style={{ width: "100%" }} size={16}>
          <Space wrap>
            <Input
              placeholder="关键字搜索"
              prefix={<SearchOutlined />}
              value={filter.keyword ?? ""}
              onChange={(e) =>
                setFilter({ keyword: e.target.value || undefined })
              }
              style={{ width: 220 }}
              allowClear
            />
            <Select
              placeholder="日志等级"
              value={filter.level}
              onChange={(v) => setFilter({ level: v })}
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
              value={filter.serverId ?? ""}
              onChange={(e) =>
                setFilter({ serverId: e.target.value || undefined })
              }
              style={{ width: 150 }}
              allowClear
            />
            <Button icon={<ReloadOutlined />} onClick={() => fetchLogs()}>
              刷新
            </Button>
          </Space>

          <Space>
            <Switch checked={autoScroll} onChange={toggleAutoScroll} /> 自动滚动
            <Button
              icon={<ExportOutlined />}
              loading={exporting}
              onClick={handleExport}
            >
              导出日志
            </Button>
            <Button danger icon={<ClearOutlined />} onClick={handleClear}>
              清空日志
            </Button>
          </Space>
        </Space>
      </Card>

      {/* 日志表格 + 下方详情面板 */}
      <Card variant="outlined">
        {error && <Text type="danger">{error}</Text>}
        <Table
          bordered
          rowKey="id"
          columns={columns}
          dataSource={entries.slice(-500)}
          loading={loading}
          pagination={{
            pageSize: 50,
            showSizeChanger: true,
            showTotal: (t) => `共 ${t} 条`,
          }}
          size="small"
          scroll={{ y: "calc(100vh - 420px)" }}
          rowClassName={(record) =>
            selectedLog?.id === record.id ? "ant-table-row-selected" : ""
          }
          onRow={(record) => ({
            onClick: () => handleRowClick(record),
            style: { cursor: "pointer" },
          })}
        />

        {/* 独立详情面板：表格下方 */}
        {renderDetailPanel()}
      </Card>
    </div>
  );
}

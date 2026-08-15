/**
 * DashboardPage - 统计面板首页
 * 所有数据通过 WebSocket 实时推送获取，无轮询
 */
import React, { useEffect, useState } from "react";
import {
  Card,
  Row,
  Col,
  Statistic,
  Table,
  Tag,
  Typography,
  Spin,
  Empty,
  Progress,
  Tooltip,
} from "antd";
import {
  CloudServerOutlined,
  TeamOutlined,
  ThunderboltOutlined,
  ArrowUpOutlined,
  ArrowDownOutlined,
  RiseOutlined,
  FallOutlined,
  ApiOutlined,
} from "@ant-design/icons";
import { useServerStore } from "../../store/useServerStore.js";
import { useClientStore } from "../../store/useClientStore.js";
import type { ServerConfig } from "../../types/index.js";

const { Title, Text } = Typography;

export function DashboardPage(): React.ReactElement {
  const {
    list: servers,
    runtimes,
    fetchServers,
    fetchRuntimes,
    loading: serverLoading,
  } = useServerStore();
  const {
    list: clients,
    fetchClients,
  } = useClientStore();
  const [refreshing, setRefreshing] = useState(false);

  // 初始化：首次加载数据（WebSocket 连接由 App.tsx 全局管理）
  useEffect(() => {
    fetchServers();
    fetchRuntimes();
    fetchClients();
  }, []);

  // 手动刷新（仅触发动画，数据由 Socket 推送保证最新）
  const handleRefresh = async () => {
    setRefreshing(true);
    await Promise.all([fetchServers(), fetchRuntimes(), fetchClients()]);
    setRefreshing(false);
  };

  const onlineCount = clients.filter((c) => c.status === "connected").length;
  const runningCount = Object.values(runtimes).filter(
    (r) => r.status === "running"
  ).length;
  const totalSent = Object.values(runtimes).reduce(
    (s, r) => s + r.sentMessages,
    0
  );
  const totalRecv = Object.values(runtimes).reduce(
    (s, r) => s + r.receivedMessages,
    0
  );
  const totalConnections = Object.values(runtimes).reduce(
    (s, r) => s + r.totalConnections,
    0
  );

  const serverColumns = [
    {
      title: '服务名称',
      dataIndex: 'name',
      key: 'name',
      align: 'center' as const,
      ellipsis: true,
      render: (v: string, r: ServerConfig) => (
        <Tooltip title={`ID: ${r.id}`}>
          <Text strong>{v || r.id || '未知'}</Text>
        </Tooltip>
      ),
    },
    {
      title: '协议',
      dataIndex: 'protocol',
      key: 'protocol',
      align: 'center' as const,
      render: (v: string) => (
        <Tag color={v === 'websocket' ? 'blue' : v === 'http' ? 'purple' : 'green'}>
          {v === 'websocket' ? 'WS' : v === 'http' ? 'HTTP' : 'S.IO'}
        </Tag>
      ),
    },
    {
      title: '地址',
      key: 'addr',
      align: 'center' as const,
      ellipsis: true,
      render: (_: unknown, r: ServerConfig) => (
        <Text code>{r.ip}:{r.port}</Text>
      ),
    },
    {
      title: '状态',
      key: 'status',
      align: 'center' as const,
      render: (_: unknown, r: ServerConfig) => {
        const rt = runtimes[r.id];
        const running = rt?.status === 'running';
        return (
          <Tag color={running ? 'success' : 'default'} icon={running ? <RiseOutlined /> : <FallOutlined />}>
            {running ? '运行中' : '已停止'}
          </Tag>
        );
      },
    },
    {
      title: '客户端',
      key: 'clients',
      align: 'center' as const,
      render: (_: unknown, r: ServerConfig) => {
        const count = runtimes[r.id]?.clientCount ?? 0;
        return <Text strong style={{ color: count > 0 ? '#52c41a' : undefined }}>{count}</Text>;
      },
    },
    {
      title: '消息(收/发)',
      key: 'messages',
      align: 'center' as const,
      render: (_: unknown, r: ServerConfig) => {
        const rt = runtimes[r.id];
        return (
          <div style={{ fontSize: 12 }}>
            <div><ArrowDownOutlined style={{ color: '#722ed1' }} /> {rt?.receivedMessages ?? 0}</div>
            <div><ArrowUpOutlined style={{ color: '#fa8c16' }} /> {rt?.sentMessages ?? 0}</div>
          </div>
        );
      },
    },
  ];

  const cardStyle = { borderRadius: 8, boxShadow: "0 2px 8px rgba(0,0,0,0.1)" };
  const cardHoverStyle = {
    ...cardStyle,
    cursor: "pointer",
    transition: "all 0.3s",
  };

  return (
    <div>
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          marginBottom: 16,
        }}
      >
        <div style={{ display: "flex", alignItems: "center" }}>
          <ApiOutlined
            style={{ fontSize: 20, marginRight: 8, color: "#1677ff" }}
          />
          <Title level={4} style={{ margin: 0, fontSize: 18 }}>
            统计面板
          </Title>
        </div>
        <Tooltip title="刷新数据">
          <Spin spinning={refreshing}>
            <ThunderboltOutlined
              style={{ fontSize: 20, cursor: "pointer", color: "#1677ff" }}
              onClick={handleRefresh}
            />
          </Spin>
        </Tooltip>
      </div>

      {/* 统计卡片 */}
      <Row gutter={[12, 12]} style={{ marginBottom: 16 }}>
        <Col xs={24} sm={12} md={6}>
          <Card style={cardHoverStyle} hoverable>
            <Statistic
              title="运行服务"
              value={runningCount}
              suffix={`/ ${servers.length}`}
              prefix={<CloudServerOutlined />}
              valueStyle={{ color: "#1677ff" }}
            />
            <Progress
              percent={
                servers.length > 0
                  ? Math.round((runningCount / servers.length) * 100)
                  : 0
              }
              size="small"
              status={runningCount === servers.length ? "success" : "active"}
              style={{ marginTop: 8 }}
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} md={6}>
          <Card style={cardHoverStyle} hoverable>
            <Statistic
              title="在线客户端"
              value={onlineCount}
              prefix={<TeamOutlined />}
              valueStyle={{ color: "#52c41a" }}
            />
            <div style={{ marginTop: 8, fontSize: 12, color: "#666" }}>
              总连接数: {totalConnections}
            </div>
          </Card>
        </Col>
        <Col xs={24} sm={12} md={6}>
          <Card style={cardHoverStyle} hoverable>
            <Statistic
              title="发送消息"
              value={totalSent}
              prefix={<ArrowUpOutlined />}
              valueStyle={{ color: "#fa8c16" }}
            />
            <div style={{ marginTop: 8, fontSize: 12, color: "#666" }}>
              总发送: {totalSent}
            </div>
          </Card>
        </Col>
        <Col xs={24} sm={12} md={6}>
          <Card style={cardHoverStyle} hoverable>
            <Statistic
              title="接收消息"
              value={totalRecv}
              prefix={<ArrowDownOutlined />}
              valueStyle={{ color: "#722ed1" }}
            />
            <div style={{ marginTop: 8, fontSize: 12, color: "#666" }}>
              总接收: {totalRecv}
            </div>
          </Card>
        </Col>
      </Row>

      {/* 服务状态总览 */}
      <Card
        title={
          <div style={{ display: "flex", alignItems: "center" }}>
            <CloudServerOutlined style={{ marginRight: 8 }} />
            服务状态总览
          </div>
        }
        variant="outlined"
        style={cardStyle}
        extra={
          <Text type="secondary" style={{ fontSize: 12 }}>
            共 {servers.length} 个服务
          </Text>
        }
      >
        {serverLoading ? (
          <Spin />
        ) : servers.length === 0 ? (
          <Empty description="暂无服务，请先添加服务" />
        ) : (
          <Table
            rowKey="id"
            columns={serverColumns}
            dataSource={servers}
            pagination={false}
            bordered
            size="small"
            scroll={{ x: "max-content" }}
            rowClassName={(record) =>
              runtimes[record.id]?.status === "running"
                ? "row-running"
                : "row-stopped"
            }
          />
        )}
      </Card>

      {/* 快速操作提示 */}
      <Card
        title="快速操作"
        style={{ ...cardStyle, marginTop: 12 }}
      >
        <div style={{ display: 'flex', alignItems: 'center', marginBottom: 8 }}>
          <span style={{ fontSize: 16, marginRight: 6 }}>💡</span>
          <Text type="secondary">提示：</Text>
        </div>
        <ul style={{ margin: 0, paddingLeft: 20, lineHeight: 1.8 }}>
          <li>点击左侧菜单可快速切换功能页面</li>
          <li>在服务管理页面可以新增、启动、停止服务</li>
          <li>在客户端管理页面可以查看连接的客户端并发送消息</li>
          <li>在日志查看页面可以实时查看系统日志</li>
        </ul>
      </Card>
    </div>
  );
}

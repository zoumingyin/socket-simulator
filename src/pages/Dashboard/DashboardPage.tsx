/**
 * DashboardPage - 统计面板首页
 */
import React, { useEffect } from 'react';
import { Card, Row, Col, Statistic, Table, Tag, Typography } from 'antd';
import {
  CloudServerOutlined,
  TeamOutlined,
  ThunderboltOutlined,
  MessageOutlined,
  ArrowUpOutlined,
  ArrowDownOutlined,
} from '@ant-design/icons';
import { useServerStore } from '../../store/useServerStore.js';
import { useClientStore } from '../../store/useClientStore.js';
import type { ServerConfig } from '../../types/index.js';

const { Title } = Typography;

export function DashboardPage(): React.ReactElement {
  const { list: servers, runtimes, fetchServers, fetchRuntimes } = useServerStore();
  const { list: clients, fetchClients } = useClientStore();

  useEffect(() => {
    fetchServers();
    fetchRuntimes();
    fetchClients();
  }, []);

  const onlineCount = clients.filter((c) => c.status === 'connected').length;
  const runningCount = Object.values(runtimes).filter((r) => r.status === 'running').length;
  const totalSent = Object.values(runtimes).reduce((s, r) => s + r.sentMessages, 0);
  const totalRecv = Object.values(runtimes).reduce((s, r) => s + r.receivedMessages, 0);

  const serverColumns = [
    { title: '服务名称', dataIndex: 'name', key: 'name',
      render: (v: string, r: ServerConfig) => v || r.id || '未知',
    },
    { title: '协议', dataIndex: 'protocol', key: 'protocol',
      render: (v: string) => <Tag color={v === 'websocket' ? 'blue' : 'green'}>{v}</Tag>,
    },
    { title: '地址', key: 'addr',
      render: (_: unknown, r: ServerConfig) => `${r.ip}:${r.port}`,
    },
    { title: '状态', key: 'status',
      render: (_: unknown, r: ServerConfig) => {
        const rt = runtimes[r.id];
        return <Tag color={rt?.status === 'running' ? 'success' : 'default'}>{rt?.status ?? 'stopped'}</Tag>;
      },
    },
    { title: '客户端', key: 'clients',
      render: (_: unknown, r: ServerConfig) => runtimes[r.id]?.clientCount ?? 0,
    },
  ];

  return (
    <div>
      <Title level={4} style={{ marginBottom: 24 }}>统计面板</Title>

      <Row gutter={[16, 16]} style={{ marginBottom: 24 }}>
        <Col span={6}>
          <Card>
            <Statistic title="运行服务" value={runningCount} suffix={`/ ${servers.length}`}
              prefix={<CloudServerOutlined />} valueStyle={{ color: '#1677ff' }} />
          </Card>
        </Col>
        <Col span={6}>
          <Card>
            <Statistic title="在线客户端" value={onlineCount}
              prefix={<TeamOutlined />} valueStyle={{ color: '#52c41a' }} />
          </Card>
        </Col>
        <Col span={6}>
          <Card>
            <Statistic title="发送消息" value={totalSent}
              prefix={<ArrowUpOutlined />} valueStyle={{ color: '#fa8c16' }} />
          </Card>
        </Col>
        <Col span={6}>
          <Card>
            <Statistic title="接收消息" value={totalRecv}
              prefix={<ArrowDownOutlined />} valueStyle={{ color: '#722ed1' }} />
          </Card>
        </Col>
      </Row>

      <Card title="服务状态总览" variant="outlined">
        <Table
          rowKey="id"
          columns={serverColumns}
          dataSource={servers}
          pagination={false}
          size="small"
        />
      </Card>
    </div>
  );
}

/**
 * App.tsx - 主应用组件
 * 包含侧边栏导航和路由出口
 */
import React, { useEffect } from 'react';
import { BrowserRouter, Routes, Route, Navigate, useNavigate, useLocation } from 'react-router-dom';
import { ConfigProvider, Layout, Menu, theme, Typography, App as AntApp } from 'antd';
import {
  DashboardOutlined,
  CloudServerOutlined,
  TeamOutlined,
  ThunderboltOutlined,
  MessageOutlined,
  FileTextOutlined,
  SettingOutlined,
} from '@ant-design/icons';
import zhCN from 'antd/locale/zh_CN';
import { useSettingsStore } from './store/useSettingsStore.js';
import { DashboardPage } from './pages/Dashboard/DashboardPage.jsx';
import { ServerManagerPage } from './pages/ServerManager/ServerManagerPage.jsx';
import { ClientManagerPage } from './pages/ClientManager/ClientManagerPage.jsx';
import { EventManagerPage } from './pages/EventManager/EventManagerPage.jsx';
import { MessageCenterPage } from './pages/MessageCenter/MessageCenterPage.jsx';
import { LogViewerPage } from './pages/LogViewer/LogViewerPage.jsx';
import { SettingsPage } from './pages/Settings/SettingsPage.jsx';

const { Sider, Content } = Layout;

const menuItems = [
  { key: '/', icon: <DashboardOutlined />, label: '仪表盘' },
  { key: '/servers', icon: <CloudServerOutlined />, label: '服务管理' },
  { key: '/clients', icon: <TeamOutlined />, label: '客户端管理' },
  { key: '/events', icon: <ThunderboltOutlined />, label: '事件管理' },
  { key: '/messages', icon: <MessageOutlined />, label: '消息中心' },
  { key: '/logs', icon: <FileTextOutlined />, label: '日志查看' },
  { key: '/settings', icon: <SettingOutlined />, label: '系统设置' },
];

function AppLayout(): React.ReactElement {
  const navigate = useNavigate();
  const location = useLocation();

  useEffect(() => {
    document.title = 'Socket 服务管理平台';
  }, []);

  return (
    <ConfigProvider
      locale={zhCN}
      theme={{
        algorithm: theme.defaultAlgorithm,
        token: { colorPrimary: '#1677ff' },
      }}
    >
      <Layout style={{ minHeight: '100vh' }}>
        <Sider width={200} theme="light">
          <div style={{ height: 48, display: 'flex', alignItems: 'center', justifyContent: 'center', borderBottom: '1px solid #f0f0f0' }}>
            <Typography.Title level={5} style={{ margin: 0, color: '#1677ff' }}>
              Socket 管理平台
            </Typography.Title>
          </div>
          <Menu
            mode="inline"
            selectedKeys={[location.pathname]}
            items={menuItems}
            style={{ height: 'calc(100vh - 48px)', borderRight: 0 }}
            onClick={({ key }) => navigate(key)}
          />
        </Sider>
        <Layout style={{ height: '100vh', overflow: 'hidden' }}>
          <Content style={{ margin: 16, height: 'calc(100vh - 32px)', overflow: 'auto', background: '#fff', borderRadius: 8, padding: 24 }}>
            <Routes>
              <Route path="/" element={<DashboardPage />} />
              <Route path="/servers" element={<ServerManagerPage />} />
              <Route path="/clients" element={<ClientManagerPage />} />
              <Route path="/events" element={<EventManagerPage />} />
              <Route path="/messages" element={<MessageCenterPage />} />
              <Route path="/logs" element={<LogViewerPage />} />
              <Route path="/settings" element={<SettingsPage />} />
              <Route path="*" element={<Navigate to="/" replace />} />
            </Routes>
          </Content>
        </Layout>
      </Layout>
    </ConfigProvider>
  );
}

export default function App(): React.ReactElement {
  return (
    <AntApp>
      <BrowserRouter>
        <AppLayout />
      </BrowserRouter>
    </AntApp>
  );
}

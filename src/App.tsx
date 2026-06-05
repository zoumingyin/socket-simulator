import React, { useEffect } from 'react';
import { BrowserRouter, Routes, Route, Navigate, useNavigate, useLocation } from 'react-router-dom';
import { ConfigProvider, Layout, Menu, theme, Typography, App as AntApp, Switch } from 'antd';
import { SunOutlined, MoonOutlined } from '@ant-design/icons';
import './responsive.css'; // 引入响应式样式
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
import { useThemeStore } from './store/useThemeStore';
import { DashboardPage } from './pages/Dashboard/DashboardPage.jsx';
import { ServerManagerPage } from './pages/ServerManager/ServerManagerPage.jsx';
import { ClientManagerPage } from './pages/ClientManager/ClientManagerPage.jsx';
import { EventManagerPage } from './pages/EventManager/EventManagerPage.jsx';
import { MessageCenterPage } from './pages/MessageCenter/MessageCenterPage.jsx';
import { LogViewerPage } from './pages/LogViewer/LogViewerPage.jsx';
import { SettingsPage } from './pages/Settings/SettingsPage.jsx';



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
  const { themeMode, toggleTheme } = useThemeStore();

  useEffect(() => {
    document.title = 'Socket 服务管理平台';
  }, []);

  const algorithm = themeMode === 'dark' ? theme.darkAlgorithm : theme.defaultAlgorithm;

  return (
    <ConfigProvider
      locale={zhCN}
      theme={{
        algorithm,
        token: { colorPrimary: '#1677ff' },
      }}
    >
      <Layout style={{ minHeight: '100vh', display: 'flex' }}>
        <Layout.Sider width={200} theme={themeMode === 'dark' ? 'dark' : 'light'} style={{ height: '100vh', overflow: 'auto' }}>
          <div style={{ height: 40, display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '0 12px', borderBottom: '1px solid #f0f0f0' }}>
            <Typography.Title level={5} style={{ margin: 0, color: themeMode === 'dark' ? '#fff' : '#1677ff', fontSize: 14 }}>
              Socket 管理平台
            </Typography.Title>
            <Switch
              checked={themeMode === 'dark'}
              onChange={toggleTheme}
              checkedChildren={<MoonOutlined style={{ fontSize: 12 }} />}
              unCheckedChildren={<SunOutlined style={{ fontSize: 12 }} />}
              size="small"
            />
          </div>
          <Menu
            mode="inline"
            selectedKeys={[location.pathname]}
            items={menuItems}
            style={{ height: 'calc(100vh - 40px)', borderRight: 0, overflow: 'auto' }}
            onClick={({ key }) => navigate(key)}
          />
        </Layout.Sider>
        <Layout style={{ display: 'flex', flexDirection: 'column', height: '100vh', overflow: 'hidden', background: themeMode === 'dark' ? '#141414' : '#f5f5f5' }}>
          <Layout.Content style={{ margin: 12, flex: 1, overflow: 'auto', background: themeMode === 'dark' ? '#141414' : '#fff', borderRadius: 8, padding: 16 }}>
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
          </Layout.Content>
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

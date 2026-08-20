import React, { useEffect } from "react";
import {
  BrowserRouter,
  Routes,
  Route,
  Navigate,
  useNavigate,
  useLocation,
} from "react-router-dom";
import {
  ConfigProvider,
  Layout,
  Menu,
  theme,
  Typography,
  App as AntApp,
  Switch,
} from "antd";
import { SunOutlined, MoonOutlined } from "@ant-design/icons";
import "./responsive.css";
import "./theme/tech-dark.css";
import {
  DashboardOutlined,
  CloudServerOutlined,
  TeamOutlined,
  ThunderboltOutlined,
  MessageOutlined,
  FileTextOutlined,
  SettingOutlined,
} from "@ant-design/icons";
import zhCN from "antd/locale/zh_CN";
import { useThemeStore } from "./store/useThemeStore";
import {
  lightThemeTokens,
  techDarkComponentTokens,
  techDarkTokens,
} from "./theme/techDark";
import { DashboardPage } from "./pages/Dashboard/DashboardPage.jsx";
import { ServerManagerPage } from "./pages/ServerManager/ServerManagerPage.jsx";
import { ClientManagerPage } from "./pages/ClientManager/ClientManagerPage.jsx";
import { EventManagerPage } from "./pages/EventManager/EventManagerPage.jsx";
import { MessageCenterPage } from "./pages/MessageCenter/MessageCenterPage.jsx";
import { LogViewerPage } from "./pages/LogViewer/LogViewerPage.jsx";
import { SettingsPage } from "./pages/Settings/SettingsPage.jsx";
import { adminSocket } from "./socket/AdminSocketManager.js";
import { bootstrapCore } from "./store/bootstrap.js";
import { useTrayBridge } from "./hooks/useTrayBridge.js";

const menuItems = [
  { key: "/", icon: <DashboardOutlined />, label: "仪表盘" },
  { key: "/servers", icon: <CloudServerOutlined />, label: "服务管理" },
  { key: "/clients", icon: <TeamOutlined />, label: "客户端管理" },
  { key: "/events", icon: <ThunderboltOutlined />, label: "事件管理" },
  { key: "/messages", icon: <MessageOutlined />, label: "消息中心" },
  { key: "/logs", icon: <FileTextOutlined />, label: "日志查看" },
  { key: "/settings", icon: <SettingOutlined />, label: "系统设置" },
];

function AppLayout(): React.ReactElement {
  const navigate = useNavigate();
  const location = useLocation();
  const { themeMode, toggleTheme } = useThemeStore();

  useEffect(() => {
    document.title = "Socket 服务管理平台";

    // 全局唯一 WebSocket 连接 — 整个应用生命周期只建立一次
    adminSocket.connect();

    // 应用级核心数据预热：先拉取 servers/runtimes/clients/events/settings，
    // 消除各页（尤其消息中心）进入时的空数据竞态。幂等，重复调用安全。
    void bootstrapCore();

    return () => {
      adminSocket.disconnect();
    };
  }, []);

  useEffect(() => {
    const root = document.documentElement;
    if (themeMode === "dark") {
      root.classList.add("theme-tech-dark");
    } else {
      root.classList.remove("theme-tech-dark");
    }
  }, [themeMode]);

  // 托盘菜单事件（Tauri 桌面环境）抽为独立 hook，保持布局组件整洁
  useTrayBridge();

  const isDark = themeMode === "dark";
  const algorithm = isDark ? theme.darkAlgorithm : theme.defaultAlgorithm;

  return (
    <ConfigProvider
      locale={zhCN}
      theme={{
        algorithm,
        token: isDark
          ? { ...techDarkTokens }
          : { ...lightThemeTokens },
        components: isDark
          ? { ...techDarkComponentTokens }
          : {
              Card: { colorBgContainer: "#ffffff" },
              Table: { colorBgContainer: "#ffffff", headerColor: "#000000" },
              Modal: { contentBg: "#ffffff" },
              Drawer: { colorBgElevated: "#ffffff" },
            },
      }}
    >
      <Layout style={{ minHeight: "100vh", display: "flex" }}>
        <Layout.Sider
          width={200}
          theme={isDark ? "dark" : "light"}
          className={isDark ? "tech-shell-sider" : undefined}
          style={{ height: "100vh", overflow: "auto" }}
        >
          <div
            className={isDark ? "tech-shell-brand" : undefined}
            style={{
              height: 44,
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between",
              padding: "0 12px",
              borderBottom: `1px solid ${isDark ? "transparent" : "#f0f0f0"}`,
            }}
          >
            <Typography.Title
              level={5}
              style={{
                margin: 0,
                color: isDark ? "#00d4ff" : "#00a67e",
                fontSize: 13,
                fontWeight: 600,
              }}
            >
              Socket 管理平台
            </Typography.Title>
            <Switch
              checked={isDark}
              onChange={toggleTheme}
              checkedChildren={<MoonOutlined style={{ fontSize: 12 }} />}
              unCheckedChildren={<SunOutlined style={{ fontSize: 12 }} />}
              size="small"
            />
          </div>
          <Menu
            mode="inline"
            theme={isDark ? "dark" : "light"}
            selectedKeys={[location.pathname]}
            items={menuItems}
            style={{
              height: "calc(100vh - 44px)",
              borderRight: 0,
              overflow: "auto",
              background: "transparent",
            }}
            onClick={({ key }) => navigate(key)}
          />
        </Layout.Sider>
        <Layout
          style={{
            display: "flex",
            flexDirection: "column",
            height: "100vh",
            overflow: "hidden",
            background: isDark ? "#070b14" : "#f5f5f5",
          }}
        >
          <Layout.Content
            className={isDark ? "tech-shell-content" : undefined}
            style={{
              margin: 12,
              flex: 1,
              overflow: "auto",
              background: isDark ? "#0c1220" : "#fff",
              borderRadius: 10,
              padding: 16,
            }}
          >
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

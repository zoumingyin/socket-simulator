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
  message,
} from "antd";
import { SunOutlined, MoonOutlined } from "@ant-design/icons";
import "./responsive.css";
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
import { DashboardPage } from "./pages/Dashboard/DashboardPage.jsx";
import { ServerManagerPage } from "./pages/ServerManager/ServerManagerPage.jsx";
import { ClientManagerPage } from "./pages/ClientManager/ClientManagerPage.jsx";
import { EventManagerPage } from "./pages/EventManager/EventManagerPage.jsx";
import { MessageCenterPage } from "./pages/MessageCenter/MessageCenterPage.jsx";
import { LogViewerPage } from "./pages/LogViewer/LogViewerPage.jsx";
import { SettingsPage } from "./pages/Settings/SettingsPage.jsx";
import { adminSocket } from "./socket/AdminSocketManager.js";
import { apiFetch } from "./api/client.js";

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

    return () => {
      adminSocket.disconnect();
    };
  }, []);

  // 监听托盘菜单事件（仅 Tauri 桌面环境）
  useEffect(() => {
    let cancelled = false;
    let unlistenFns: (() => void)[] = [];

    const setupTrayListeners = async () => {
      try {
        // 动态导入 Tauri event API（非 Tauri 环境下会失败）
        const { listen } = await import("@tauri-apps/api/event");

        if (cancelled) return;

        // 启动全部服务
        const unlistenStart = await listen("tray-start-all", async () => {
          message.info("正在启动全部服务...");
          try {
            const res = await apiFetch("/api/server/start-all", {
              method: "POST",
            });
            if (res.success) {
              message.success("全部服务启动成功");
            } else {
              message.error(`启动失败: ${res.error || "未知错误"}`);
            }
          } catch (err: unknown) {
            const error = err instanceof Error ? err : new Error(String(err));
            message.error(`启动失败: ${error.message}`);
          }
        });

        if (cancelled) {
          unlistenStart();
          return;
        }

        // 停止全部服务
        const unlistenStop = await listen("tray-stop-all", async () => {
          message.info("正在停止全部服务...");
          try {
            const res = await apiFetch("/api/server/stop-all", {
              method: "POST",
            });
            if (res.success) {
              message.success("全部服务停止成功");
            } else {
              message.error(`停止失败: ${res.error || "未知错误"}`);
            }
          } catch (err: unknown) {
            const error = err instanceof Error ? err : new Error(String(err));
            message.error(`停止失败: ${error.message}`);
          }
        });

        if (cancelled) {
          unlistenStop();
          return;
        }

        // 重启全部服务
        const unlistenRestart = await listen("tray-restart-all", async () => {
          message.info("正在重启全部服务...");
          try {
            const res = await apiFetch("/api/server/restart-all", {
              method: "POST",
            });
            if (res.success) {
              message.success("全部服务重启成功");
            } else {
              message.error(`重启失败: ${res.error || "未知错误"}`);
            }
          } catch (err: unknown) {
            const error = err instanceof Error ? err : new Error(String(err));
            message.error(`重启失败: ${error.message}`);
          }
        });

        unlistenFns = [unlistenStart, unlistenStop, unlistenRestart];
      } catch {
        // 非 Tauri 环境（浏览器开发），忽略
        return;
      }
    };

    setupTrayListeners();

    return () => {
      cancelled = true;
      unlistenFns.forEach((fn) => fn());
    };
  }, []);

  const isDark = themeMode === "dark";
  const algorithm = isDark ? theme.darkAlgorithm : theme.defaultAlgorithm;

  return (
    <ConfigProvider
      locale={zhCN}
      theme={{
        algorithm,
        token: {
          colorPrimary: "#1677ff",
          colorBgBase: isDark ? "#141414" : "#ffffff",
          colorTextBase: isDark ? "#ffffff" : "#000000",
        },
        components: {
          Card: {
            colorBgContainer: isDark ? "#1f1f1f" : "#ffffff",
          },
          Table: {
            colorBgContainer: isDark ? "#1f1f1f" : "#ffffff",
            headerColor: isDark ? "#ffffff" : "#000000",
          },
          Modal: {
            colorBgElevated: isDark ? "#1f1f1f" : "#ffffff",
          },
          Drawer: {
            colorBgElevated: isDark ? "#1f1f1f" : "#ffffff",
          },
        },
      }}
    >
      <Layout style={{ minHeight: "100vh", display: "flex" }}>
        <Layout.Sider
          width={200}
          theme={isDark ? "dark" : "light"}
          style={{ height: "100vh", overflow: "auto" }}
        >
          <div
            style={{
              height: 40,
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between",
              padding: "0 12px",
              borderBottom: `1px solid ${isDark ? "#333" : "#f0f0f0"}`,
            }}
          >
            <Typography.Title
              level={5}
              style={{
                margin: 0,
                color: isDark ? "#fff" : "#1677ff",
                fontSize: 14,
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
            selectedKeys={[location.pathname]}
            items={menuItems}
            style={{
              height: "calc(100vh - 40px)",
              borderRight: 0,
              overflow: "auto",
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
            background: isDark ? "#0a0a0a" : "#f5f5f5",
          }}
        >
          <Layout.Content
            style={{
              margin: 12,
              flex: 1,
              overflow: "auto",
              background: isDark ? "#141414" : "#fff",
              borderRadius: 8,
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

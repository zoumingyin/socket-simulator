/**
 * SettingsPage - 系统设置页面（优化版）
 * 使用 Tabs 组织设置项，提升用户体验
 */
import React, { useEffect, useLayoutEffect } from 'react';
import {
  Tabs, Form, Input, InputNumber, Switch, Button, Space,
  Typography, Upload, message, Card, Divider,
} from 'antd';
import { SaveOutlined, ImportOutlined, ExportOutlined, SettingOutlined } from '@ant-design/icons';
import { useSettingsStore } from '../../store/useSettingsStore.js';

const { Title, Paragraph } = Typography;
const { TextArea } = Input;

export function SettingsPage(): React.ReactElement {
  const [messageApi, contextHolder] = message.useMessage();
  const {
    systemSettings, windowConfig, loading, error,
    fetchSettings, updateSystemSettings, updateWindowConfig, exportConfig, importConfig,
  } = useSettingsStore();
  const [form] = Form.useForm();
  const [windowForm] = Form.useForm();

  useEffect(() => {
    let cancelled = false;
    let retryTimer: ReturnType<typeof setTimeout>;

    const loadWithRetry = async (retry = 0) => {
      await fetchSettings();
      if (cancelled) return;
      // 如果加载失败且还有重试次数（处理后端启动延迟的竞态条件）
      const state = useSettingsStore.getState();
      if (state.error && retry < 3) {
        retryTimer = setTimeout(() => loadWithRetry(retry + 1), 2000);
      }
    };

    loadWithRetry();
    return () => {
      cancelled = true;
      clearTimeout(retryTimer);
    };
  }, []);

  // 使用 useLayoutEffect 在浏览器绘制前同步回填表单，消除默认值闪烁
  useLayoutEffect(() => {
    if (systemSettings) {
      form.setFieldsValue({
        heartbeat: systemSettings.heartbeat
          ? {
              enabled: systemSettings.heartbeat.enabled,
              pingInterval: Number(systemSettings.heartbeat.pingInterval),
              pongTimeout: Number(systemSettings.heartbeat.pongTimeout),
            }
          : { enabled: true, pingInterval: 30000, pongTimeout: 90000 },
        wss: systemSettings.wss ?? { enabled: false, certPath: '', keyPath: '' },
        // IP 名单：后端存的是 string[]，TextArea 需要字符串
        ipAccess: systemSettings.ipAccess
          ? {
              whitelist: Array.isArray(systemSettings.ipAccess.whitelist)
                ? systemSettings.ipAccess.whitelist.join('\n')
                : '',
              blacklist: Array.isArray(systemSettings.ipAccess.blacklist)
                ? systemSettings.ipAccess.blacklist.join('\n')
                : '',
            }
          : { whitelist: '', blacklist: '' },
        autoStart: systemSettings.autoStart,
        startMinimized: systemSettings.startMinimized,
        logRetentionDays: Number(systemSettings.logRetentionDays),
        maxConnectionsPerServer: Number(systemSettings.maxConnectionsPerServer),
      });
    }
    if (windowConfig) {
      windowForm.setFieldsValue(windowConfig);
    }
  }, [systemSettings, windowConfig]);

  const handleSave = async () => {
    try {
      const vals = await form.validateFields();
      // 数值字段确保存储为 number
      const normalized = {
        ...vals,
        heartbeat: vals.heartbeat ? {
          enabled: vals.heartbeat.enabled,
          pingInterval: Number(vals.heartbeat.pingInterval),
          pongTimeout: Number(vals.heartbeat.pongTimeout),
        } : vals.heartbeat,
        logRetentionDays: Number(vals.logRetentionDays),
        maxConnectionsPerServer: Number(vals.maxConnectionsPerServer),
        // IP 名单：TextArea 产出字符串，拆分为 string[]
        ipAccess: vals.ipAccess ? {
          whitelist: typeof vals.ipAccess.whitelist === 'string'
            ? vals.ipAccess.whitelist.split('\n').map((s: string) => s.trim()).filter(Boolean)
            : vals.ipAccess.whitelist,
          blacklist: typeof vals.ipAccess.blacklist === 'string'
            ? vals.ipAccess.blacklist.split('\n').map((s: string) => s.trim()).filter(Boolean)
            : vals.ipAccess.blacklist,
        } : vals.ipAccess,
      };
      await updateSystemSettings(normalized);
      messageApi.success('系统设置已保存');
    } catch (err: unknown) {
      // 区分表单验证错误和 API 错误
      if (err && typeof err === 'object' && 'errorFields' in err) {
        messageApi.error('请检查表单输入');
      } else {
        const msg = err instanceof Error ? err.message : '保存失败，请检查网络连接或后端服务是否正常';
        messageApi.error(msg);
      }
    }
  };

  const handleWindowSave = async () => {
    try {
      const vals = await windowForm.validateFields();
      await updateWindowConfig({
        width: Number(vals.width),
        height: Number(vals.height),
        maximized: vals.maximized,
      });
      messageApi.success('窗口配置已保存');
    } catch (err: unknown) {
      if (err && typeof err === 'object' && 'errorFields' in err) {
        messageApi.error('请检查表单输入');
      } else {
        const msg = err instanceof Error ? err.message : '保存失败，请检查网络连接或后端服务是否正常';
        messageApi.error(msg);
      }
    }
  };

  const handleExport = async () => {
    try {
      const config = await exportConfig();
      const blob = new Blob([JSON.stringify(config, null, 2)], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `socket-service-config-${new Date().toISOString().split('T')[0]}.json`;
      a.click();
      URL.revokeObjectURL(url);
      messageApi.success('配置已导出');
    } catch (err) {
      messageApi.error('导出失败');
    }
  };

  const handleImport = async (file: File) => {
    const reader = new FileReader();
    reader.onload = async (e) => {
      try {
        const config = JSON.parse(e.target?.result as string);
        await importConfig(config);
        messageApi.success('配置已导入');
      } catch {
        messageApi.error('配置文件格式无效');
      }
    };
    reader.readAsText(file);
    return false;
  };

  const tabItems = [
    {
      key: 'basic',
      label: '基本设置',
      forceRender: true,
      children: (
        <Form form={form} layout="vertical" style={{ maxWidth: 600, marginTop: 16 }}>
          <Form.Item 
            name="autoStart" 
            label="启动时自动启动服务" 
            valuePropName="checked"
            tooltip="开启后，应用启动时会自动启动所有配置了自动启动的服务"
          >
            <Switch />
          </Form.Item>
          <Form.Item 
            name="startMinimized" 
            label="启动时最小化到托盘" 
            valuePropName="checked"
            tooltip="开启后，应用启动时会最小化到系统托盘"
          >
            <Switch />
          </Form.Item>
          <Form.Item 
            name="logRetentionDays" 
            label="日志保留天数（天）"
            rules={[{ required: true, message: '请输入日志保留天数' }]}
            tooltip="超过保留天数的日志将被自动清理"
          >
            <InputNumber min={1} max={365} style={{ width: 120 }} />
          </Form.Item>
          <Form.Item 
            name="maxConnectionsPerServer" 
            label="单服务最大连接数"
            rules={[{ required: true, message: '请输入最大连接数' }]}
            tooltip="每个服务允许的最大客户端连接数，防止资源耗尽"
          >
            <InputNumber min={1} max={10000} style={{ width: 200 }} />
          </Form.Item>
        </Form>
      ),
    },
    {
      key: 'heartbeat',
      label: '心跳配置',
      forceRender: true,
      children: (
        <Form form={form} layout="vertical" style={{ maxWidth: 600, marginTop: 16 }}>
          <Form.Item 
            name={['heartbeat', 'enabled']} 
            label="启用心跳检测" 
            valuePropName="checked"
            tooltip="开启后，定期向客户端发送心跳包检测连接状态"
          >
            <Switch />
          </Form.Item>
          <Form.Item 
            name={['heartbeat', 'pingInterval']} 
            label="Ping 间隔（ms）"
            rules={[{ required: true, message: '请输入 Ping 间隔' }]}
            tooltip="向客户端发送心跳包的间隔时间"
          >
            <InputNumber min={5000} max={300000} style={{ width: 200 }} />
          </Form.Item>
          <Form.Item 
            name={['heartbeat', 'pongTimeout']} 
            label="Pong 超时（ms）"
            rules={[{ required: true, message: '请输入 Pong 超时' }]}
            tooltip="等待客户端响应心跳的超时时间，超过此时间将断开连接"
          >
            <InputNumber min={10000} max={600000} style={{ width: 200 }} />
          </Form.Item>
        </Form>
      ),
    },
    {
      key: 'wss',
      label: 'WSS 配置',
      forceRender: true,
      children: (
        <Form form={form} layout="vertical" style={{ maxWidth: 600, marginTop: 16 }}>
          <Form.Item 
            name={['wss', 'enabled']} 
            label="启用 WSS (WebSocket Secure)" 
            valuePropName="checked"
            tooltip="开启后，WebSocket 连接将使用 SSL/TLS 加密"
          >
            <Switch />
          </Form.Item>
          <Form.Item 
            name={['wss', 'certPath']} 
            label="SSL 证书路径 (cert.pem)"
            tooltip="SSL 证书文件路径，支持 PEM 格式"
          >
            <Input placeholder="C:/certs/cert.pem" />
          </Form.Item>
          <Form.Item 
            name={['wss', 'keyPath']} 
            label="SSL 密钥路径 (key.pem)"
            tooltip="SSL 私钥文件路径，支持 PEM 格式"
          >
            <Input placeholder="C:/certs/key.pem" />
          </Form.Item>
        </Form>
      ),
    },
    {
      key: 'ipAccess',
      label: 'IP 访问控制',
      forceRender: true,
      children: (
        <Form form={form} layout="vertical" style={{ maxWidth: 600, marginTop: 16 }}>
          <Form.Item 
            name={['ipAccess', 'whitelist']} 
            label="IP 白名单"
            tooltip="只允许白名单中的 IP 地址连接，每行一个 IP 地址。留空表示不限制"
          >
            <TextArea rows={4} placeholder="127.0.0.1&#10;::1&#10;192.168.1.0/24" />
          </Form.Item>
          <Form.Item 
            name={['ipAccess', 'blacklist']} 
            label="IP 黑名单"
            tooltip="黑名单中的 IP 地址将被拒绝连接，每行一个 IP 地址"
          >
            <TextArea rows={4} placeholder="192.168.1.100&#10;10.0.0.50" />
          </Form.Item>
        </Form>
      ),
    },
    {
      key: 'window',
      label: '窗口配置',
      forceRender: true,
      children: (
        <Form form={windowForm} layout="vertical" style={{ maxWidth: 600, marginTop: 16 }}>
          <Form.Item 
            name="width" 
            label="窗口宽度（px）"
            rules={[{ required: true, message: '请输入窗口宽度' }]}
          >
            <InputNumber min={800} max={3840} style={{ width: 180 }} />
          </Form.Item>
          <Form.Item 
            name="height" 
            label="窗口高度（px）"
            rules={[{ required: true, message: '请输入窗口高度' }]}
          >
            <InputNumber min={600} max={2160} style={{ width: 180 }} />
          </Form.Item>
          <Form.Item 
            name="maximized" 
            label="启动时最大化" 
            valuePropName="checked"
            tooltip="开启后，应用启动时会最大化窗口"
          >
            <Switch />
          </Form.Item>
          <Form.Item>
            <Button type="primary" onClick={handleWindowSave} loading={loading}>
              保存窗口配置
            </Button>
          </Form.Item>
        </Form>
      ),
    },
    {
      key: 'importExport',
      label: '导入/导出',
      children: (
        <div style={{ marginTop: 16 }}>
          <Paragraph>导出或导入应用的全部配置，包括服务配置、事件配置、消息模板等。</Paragraph>
          <Space size="middle">
            <Button icon={<ExportOutlined />} onClick={handleExport} size="large">
              导出全部配置
            </Button>
            <Upload accept=".json" showUploadList={false} beforeUpload={handleImport}>
              <Button icon={<ImportOutlined />} size="large">
                导入全部配置
              </Button>
            </Upload>
          </Space>
        </div>
      ),
    },
  ];

  return (
    <div>
      {contextHolder}
      <div style={{ display: 'flex', alignItems: 'center', marginBottom: 16 }}>
        <SettingOutlined style={{ fontSize: 20, marginRight: 8, color: '#1677ff' }} />
        <Title level={4} style={{ margin: 0, fontSize: 18 }}>系统设置</Title>
      </div>
      
      {error && <Paragraph type="danger">{error}</Paragraph>}
      
      <Card variant="outlined" style={{ boxShadow: '0 2px 8px rgba(0,0,0,0.1)' }}>
        <Tabs 
          items={tabItems} 
          type="card"
          destroyOnHidden={false}
        />
      </Card>
      
      <Divider />
      
      <div style={{ textAlign: 'right' }}>
        <Button 
          type="primary" 
          icon={<SaveOutlined />} 
          onClick={handleSave} 
          loading={loading}
          size="large"
        >
          保存系统设置
        </Button>
      </div>
    </div>
  );
}

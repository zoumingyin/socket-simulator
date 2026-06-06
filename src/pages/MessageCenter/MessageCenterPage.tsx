/**
 * MessageCenterPage - 消息中心页面
 * 支持广播、指定客户端、指定分组；支持 Text/JSON 类型；支持 JSON 格式化/校验/压缩
 * 事件名称改为下拉选择，数据源为事件管理列表
 */
import React, { useEffect, useState } from 'react';
import {
  Card, Space, Typography, Radio, Input, Button, Select,
  Switch, Divider, Tag,
  message,
} from 'antd';
import { SendOutlined, FormatPainterOutlined, CheckCircleOutlined, CompressOutlined } from '@ant-design/icons';
import type { MessageType, SendMessageRequest, MessageTargetType } from '../../types/index.js';
import { useMessageStore } from '../../store/useMessageStore.js';
import { useClientStore } from '../../store/useClientStore.js';
import { useServerStore } from '../../store/useServerStore.js';
import { useEventStore } from '../../store/useEventStore.js';

const { Title, Text, Paragraph } = Typography;
const { TextArea } = Input;

export function MessageCenterPage(): React.ReactElement {
  const [messageApi, contextHolder] = message.useMessage();
  const {
    sending, error, sendMessage, broadcast, formatJSON, validateJSON, minifyJSON,
  } = useMessageStore();
  const { list: clients, fetchClients } = useClientStore();
  const { list: servers } = useServerStore();
  const { list: events, fetchEvents } = useEventStore();

  const [targetType, setTargetType] = useState<MessageTargetType>('broadcast');
  const [serverId, setServerId] = useState<string | undefined>(undefined);
  const [event, setEvent] = useState('');
  const [messageType, setMessageType] = useState<MessageType>('json');
  const [content, setContent] = useState('{"msg":"hello"}');
  const [targetId, setTargetId] = useState<string>('');

  useEffect(() => { fetchClients(); fetchEvents(); }, []);

  const handleSend = async () => {
    if (!serverId || !event) { messageApi.warning('请填写服务和事件'); return; }
    if (messageType === 'json') {
      const v = validateJSON(content);
      if (!v.valid) { messageApi.error(`JSON 无效: ${v.error}`); return; }
    }
    try {
      if (targetType === 'broadcast') {
        await broadcast({ serverId, event, messageType, content });
      } else {
        await sendMessage({ serverId, targetType, targetId: targetId || undefined, event, messageType, content });
      }
      messageApi.success('发送成功');
    } catch {
      messageApi.error('发送失败');
    }
  };

  const handleFormat = () => {
    const r = formatJSON(content);
    if (r.ok) setContent(r.formatted!); else messageApi.error(r.error);
  };
  const handleValidate = () => {
    const r = validateJSON(content);
    r.valid ? messageApi.success('JSON 合法') : messageApi.error(r.error);
  };
  const handleMinify = () => {
    const r = minifyJSON(content);
    if (r.ok) setContent(r.minified!); else messageApi.error(r.error);
  };

  /** 过滤出当前选中服务下的已启用事件 */
  const filteredEvents = serverId
    ? events.filter(e => e.serverId === serverId && e.status === 'enabled')
    : events.filter(e => e.status === 'enabled');

  return (
    <div>
      {contextHolder}
      <Title level={4} style={{ marginBottom: 16, fontSize: 18 }}>消息中心</Title>

      <Card title="发送消息" variant="outlined" style={{ marginBottom: 24 }}>
        <Space direction="vertical" style={{ width: '100%' }} size={16}>

          <Space>
            <Text>目标类型：</Text>
            <Radio.Group value={targetType} onChange={e => setTargetType(e.target.value)}>
              <Radio.Button value="broadcast">广播</Radio.Button>
              <Radio.Button value="client">指定客户端</Radio.Button>
            </Radio.Group>
          </Space>

          <Space>
            <Text>服务：</Text>
            <Select
              value={serverId}
              onChange={v => { setServerId(v ?? undefined); setEvent(''); }}
              style={{ width: 220 }}
              placeholder="选择服务"
              allowClear
            >
              {servers.filter(s => s.id).map(s => (
                <Select.Option key={s.id!} value={s.id!}>
                  {s.name} ({s.protocol})
                </Select.Option>
              ))}
            </Select>
          </Space>

          {targetType === 'client' && (
            <Space>
              <Text>客户端：</Text>
              <Select
                value={targetId || undefined}
                onChange={v => setTargetId(v)}
                style={{ width: 300 }}
                placeholder={!serverId ? '请先选择服务' : '选择客户端'}
                allowClear
                showSearch
                optionFilterProp="children"
                disabled={!serverId}
              >
                {clients
                  .filter(c => c.status === 'connected' && (!serverId || c.serverId === serverId))
                  .map(c => (
                    <Select.Option key={c.id} value={c.id}>
                      {c.socketId}（{c.ipAddress}）
                    </Select.Option>
                  ))}
              </Select>
            </Space>
          )}

          <Space>
            <Text>事件名称：</Text>
            <Select
              value={event || undefined}
              onChange={v => setEvent(v)}
              style={{ width: 220 }}
              placeholder="选择事件"
              allowClear
              showSearch
              optionFilterProp="children"
            >
              {filteredEvents.map(e => (
                <Select.Option key={e.id} value={e.name}>
                  {e.name}{e.description ? ` - ${e.description}` : ''}
                  {e.pollingEnabled && <Tag color="blue" style={{ marginLeft: 4 }}>轮询中</Tag>}
                </Select.Option>
              ))}
              {filteredEvents.length === 0 && (
                <Select.Option value="__empty__" disabled>暂无已启用事件，请先在事件管理中添加</Select.Option>
              )}
            </Select>
          </Space>

          <Space>
            <Text>消息类型：</Text>
            <Radio.Group value={messageType} onChange={e => setMessageType(e.target.value)}>
              <Radio value="text">Text</Radio>
              <Radio value="json">JSON</Radio>
            </Radio.Group>
          </Space>

          <div>
            <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 8 }}>
              <Text>消息内容：</Text>
              {messageType === 'json' && (
                <Space>
                  <Button size="small" icon={<FormatPainterOutlined />} onClick={handleFormat}>格式化</Button>
                  <Button size="small" icon={<CheckCircleOutlined />} onClick={handleValidate}>校验</Button>
                  <Button size="small" icon={<CompressOutlined />} onClick={handleMinify}>压缩</Button>
                </Space>
              )}
            </div>
            <TextArea
              value={content}
              onChange={e => setContent(e.target.value)}
              rows={8}
              placeholder={messageType === 'json' ? '输入 JSON...' : '输入文本消息...'}
            />
          </div>

          <Button type="primary" icon={<SendOutlined />} onClick={handleSend} loading={sending}>发送消息</Button>
          {error && <Text type="danger">{error}</Text>}
        </Space>
      </Card>
    </div>
  );
}

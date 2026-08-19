/**
 * SceneManager - 场景编排（P1-3）
 *
 * 场景 = 有序服务组；一键启动按 serverIds 顺序、停止逆序。
 * 仅做编排与启停，不修改服务本身配置。
 */
import React, { useCallback, useEffect, useState } from 'react';
import {
  Button,
  Empty,
  Form,
  Input,
  Modal,
  Popconfirm,
  Select,
  Space,
  Spin,
  Tag,
  Typography,
} from 'antd';
import { PlusOutlined, ReloadOutlined } from '@ant-design/icons';
import type { MessageInstance } from 'antd/es/message/interface';
import { api } from '../../../api/index.js';
import type { SceneConfig, ServerConfig } from '../../../types/index.js';

const { Text } = Typography;

interface SceneManagerProps {
  open: boolean;
  onClose: () => void;
  /** 可用服务（供场景选组） */
  servers: ServerConfig[];
  /** 启停后通知父级刷新运行时 */
  onChanged?: () => void;
  messageApi: MessageInstance;
}

export function SceneManager({
  open,
  onClose,
  servers,
  onChanged,
  messageApi,
}: SceneManagerProps): React.ReactElement {
  const [scenes, setScenes] = useState<SceneConfig[]>([]);
  const [loading, setLoading] = useState(false);
  const [actingId, setActingId] = useState<string | null>(null);

  // 新建/编辑
  const [editOpen, setEditOpen] = useState(false);
  const [editing, setEditing] = useState<SceneConfig | null>(null);
  const [saving, setSaving] = useState(false);
  const [form] = Form.useForm();

  const serverOptions = servers.map((s) => ({ value: s.id, label: s.name || s.id }));

  const load = useCallback(async () => {
    setLoading(true);
    const res = await api.scenes.list();
    setLoading(false);
    if (res.success && res.data) setScenes(res.data);
    else messageApi.error('加载场景失败：' + (res.error ?? '未知错误'));
  }, [messageApi]);

  useEffect(() => {
    if (open) load();
  }, [open, load]);

  const openCreate = () => {
    setEditing(null);
    form.resetFields();
    setEditOpen(true);
  };

  const openEdit = (scene: SceneConfig) => {
    setEditing(scene);
    form.setFieldsValue({
      name: scene.name,
      description: scene.description,
      serverIds: scene.serverIds,
    });
    setEditOpen(true);
  };

  const handleSave = async () => {
    const vals = await form.validateFields();
    setSaving(true);
    const body = {
      name: vals.name,
      description: vals.description || '',
      serverIds: vals.serverIds ?? [],
      enabled: true,
    };
    const res = editing
      ? await api.scenes.update({ ...editing, ...body })
      : await api.scenes.add(body);
    setSaving(false);
    if (res.success) {
      messageApi.success(editing ? '场景已更新' : '场景已创建');
      setEditOpen(false);
      load();
    } else {
      messageApi.error('保存失败：' + (res.error ?? '未知错误'));
    }
  };

  const handleRemove = async (id: string) => {
    const res = await api.scenes.remove(id);
    if (res.success) {
      messageApi.success('场景已删除');
      load();
    } else {
      messageApi.error('删除失败：' + (res.error ?? '未知错误'));
    }
  };

  const handleStart = async (scene: SceneConfig) => {
    const id = scene.id ?? '';
    setActingId(id);
    const res = await api.scenes.start(id);
    setActingId(null);
    if (!res.success) {
      messageApi.error('启动失败：' + (res.error ?? '未知错误'));
      return;
    }
    const failed = (res.data ?? []).filter((r) => !r.success);
    if (failed.length === 0) {
      messageApi.success(`场景「${scene.name}」已全部启动`);
    } else {
      messageApi.warning(
        `场景「${scene.name}」启动完成，${failed.length} 个服务失败：` +
          failed.map((f) => f.serverId).join('、')
      );
    }
    onChanged?.();
  };

  const handleStop = async (scene: SceneConfig) => {
    const id = scene.id ?? '';
    setActingId(id);
    const res = await api.scenes.stop(id);
    setActingId(null);
    if (res.success) {
      messageApi.success(`场景「${scene.name}」已停止 ${res.data?.stopped ?? 0} 个服务`);
    } else {
      messageApi.error('停止失败：' + (res.error ?? '未知错误'));
    }
    onChanged?.();
  };

  const nameOf = (id: string): string => servers.find((s) => s.id === id)?.name || id;

  return (
    <>
      <Modal
        title="场景编排"
        open={open}
        onCancel={onClose}
        footer={
          <Space>
            <Button icon={<ReloadOutlined />} onClick={load}>
              刷新
            </Button>
            <Button type="primary" icon={<PlusOutlined />} onClick={openCreate}>
              新建场景
            </Button>
          </Space>
        }
        width={720}
      >
        <Spin spinning={loading}>
          {scenes.length === 0 && !loading ? (
            <Empty description="暂无场景，点击「新建场景」把一组服务编排为可一键启停的组" />
          ) : (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
              {scenes.map((scene) => (
                <div
                  key={scene.id}
                  style={{
                    border: '1px solid #e5e7eb',
                    borderRadius: 8,
                    padding: '12px 16px',
                  }}
                >
                  <div
                    style={{
                      display: 'flex',
                      justifyContent: 'space-between',
                      alignItems: 'center',
                      gap: 8,
                    }}
                  >
                    <div style={{ minWidth: 0 }}>
                      <Text strong style={{ fontSize: 14 }}>{scene.name}</Text>
                      {scene.description ? (
                        <Text type="secondary" style={{ marginLeft: 8, fontSize: 12 }}>
                          {scene.description}
                        </Text>
                      ) : null}
                    </div>
                    <Space size={4}>
                      <Button size="small" type="primary" loading={actingId === scene.id}
                        onClick={() => handleStart(scene)}>
                        启动
                      </Button>
                      <Button size="small" danger loading={actingId === scene.id}
                        onClick={() => handleStop(scene)}>
                        停止
                      </Button>
                      <Button size="small" onClick={() => openEdit(scene)}>编辑</Button>
                      <Popconfirm title="删除该场景？" description="仅移除编排，不停服务"
                        onConfirm={() => handleRemove(scene.id ?? '')}>
                        <Button size="small" danger type="text">删除</Button>
                      </Popconfirm>
                    </Space>
                  </div>
                  <div style={{ marginTop: 8, display: 'flex', flexWrap: 'wrap', gap: 6 }}>
                    {(scene.serverIds ?? []).length === 0 ? (
                      <Text type="secondary" style={{ fontSize: 12 }}>（未选择服务）</Text>
                    ) : (
                      (scene.serverIds ?? []).map((sid, i) => (
                        <Tag key={sid} color="blue" style={{ margin: 0 }}>
                          {i + 1}. {nameOf(sid)}
                        </Tag>
                      ))
                    )}
                  </div>
                </div>
              ))}
            </div>
          )}
        </Spin>
      </Modal>

      <Modal
        title={editing ? '编辑场景' : '新建场景'}
        open={editOpen}
        onCancel={() => setEditOpen(false)}
        onOk={handleSave}
        confirmLoading={saving}
        okText="保存"
        width={560}
      >
        <Form form={form} layout="vertical">
          <Form.Item name="name" label="场景名称" rules={[{ required: true, message: '请输入场景名称' }]}>
            <Input placeholder="如：开发环境 / 测试环境" />
          </Form.Item>
          <Form.Item name="description" label="描述">
            <Input placeholder="可选" />
          </Form.Item>
          <Form.Item
            name="serverIds"
            label="服务（按选择顺序启动，停止时逆序）"
            rules={[{ required: true, message: '请选择至少一个服务' }]}
          >
            <Select
              mode="multiple"
              placeholder="依次选择服务，顺序即启动顺序"
              options={serverOptions}
              optionFilterProp="label"
            />
          </Form.Item>
        </Form>
      </Modal>
    </>
  );
}

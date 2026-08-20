import { useMemo, useRef, useState } from 'react';
import type { Key } from 'react';
import { Alert, Button, Checkbox, Modal, Space, Table, Tag, Typography, message } from 'antd';
import { UploadOutlined, FolderOutlined } from '@ant-design/icons';
import { parseSwaggerToRules } from './importSwagger.js';
import type { MockRule } from '../../types/index.js';

const { Text } = Typography;

/** 方法标签颜色 */
const METHOD_COLOR: Record<string, string> = {
  GET: 'blue', POST: 'green', PUT: 'orange', DELETE: 'red', PATCH: 'purple', HEAD: 'cyan', OPTIONS: 'default',
};

/**
 * SwaggerImportModal — 导入 Swagger/OpenAPI 文档生成 Mock 规则
 *
 * 支持粘贴 JSON 或选择 .json 文件（OpenAPI 3.x / Swagger 2.0）；
 * 解析后按文档 tags 分组预览，可勾选任意分组/行，仅导入选中的规则。
 */
export function SwaggerImportModal({
  open,
  onCancel,
  onImport,
}: {
  open: boolean;
  onCancel: () => void;
  onImport: (rules: MockRule[]) => Promise<void>;
}) {
  const [text, setText] = useState('');
  const [parsed, setParsed] = useState<MockRule[] | null>(null);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [parsing, setParsing] = useState(false);
  const [importing, setImporting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const reset = () => {
    setText('');
    setParsed(null);
    setSelectedIds(new Set());
    setError(null);
    setImporting(false);
  };

  const handleClose = () => {
    reset();
    onCancel();
  };

  const handleParse = () => {
    if (!text.trim()) {
      setError('请先粘贴文档内容或选择 .json 文件');
      return;
    }
    setParsing(true);
    setError(null);
    try {
      const rules = parseSwaggerToRules(text);
      setParsed(rules);
      // 解析成功后默认全选，可直接导入
      setSelectedIds(new Set(rules.map((r) => r.id ?? '')));
      message.success(`解析成功：提取 ${rules.length} 条接口`);
    } catch (e) {
      setParsed(null);
      setSelectedIds(new Set());
      setError((e as Error).message);
    } finally {
      setParsing(false);
    }
  };

  const rowKey = (r: MockRule) => r.id ?? `${r.method}-${r.pathPattern}`;
  const selectedRules = useMemo(
    () => parsed?.filter((r) => selectedIds.has(rowKey(r))) ?? [],
    [parsed, selectedIds],
  );

  /** 按 group（tags）分组预览；无分组归「未分组」 */
  const groups = useMemo(() => {
    if (!parsed) return [];
    const list: Array<{ group: string; rules: MockRule[] }> = [];
    const none: MockRule[] = [];
    const idx = new Map<string, number>();
    for (const r of parsed) {
      const g = (r.group ?? '').trim();
      if (!g) {
        none.push(r);
        continue;
      }
      const i = idx.get(g);
      if (i === undefined) {
        idx.set(g, list.length);
        list.push({ group: g, rules: [r] });
      } else {
        list[i].rules.push(r);
      }
    }
    return [...list, ...(none.length > 0 ? [{ group: '未分组', rules: none }] : [])];
  }, [parsed]);

  const rowSelection = {
    selectedRowKeys: Array.from(selectedIds),
    onChange: (keys: Key[]) => setSelectedIds(new Set(keys.map(String))),
  };

  const groupChecked = (list: MockRule[]) =>
    list.length > 0 && list.every((r) => selectedIds.has(rowKey(r)));
  const groupIndeterminate = (list: MockRule[]) =>
    list.some((r) => selectedIds.has(rowKey(r))) && !groupChecked(list);
  const toggleGroup = (list: MockRule[], checked: boolean) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      for (const r of list) {
        const id = rowKey(r);
        if (checked) next.add(id);
        else next.delete(id);
      }
      return next;
    });
  };

  const handleImport = async () => {
    if (selectedRules.length === 0) return;
    setImporting(true);
    try {
      await onImport(selectedRules);
      message.success(`已导入 ${selectedRules.length} 条规则`);
      handleClose();
    } catch (e) {
      message.error(`导入失败：${(e as Error).message}`);
      setImporting(false);
    }
  };

  const columns = [
    {
      title: '方法',
      dataIndex: 'method',
      width: 80,
      render: (m: string) => <Tag color={METHOD_COLOR[m] ?? 'default'} style={{ margin: 0 }}>{m}</Tag>,
    },
    { title: '路径', dataIndex: 'pathPattern', ellipsis: true },
    { title: '状态码', dataIndex: 'responseStatusCode', width: 72 },
    { title: '规则名', dataIndex: 'name', ellipsis: true },
  ];

  return (
    <Modal
      title="导入 Swagger / OpenAPI 文档"
      open={open}
      onCancel={handleClose}
      width={760}
      centered
      footer={
        <Space>
          <Button onClick={handleClose}>取消</Button>
          <Button loading={parsing} onClick={handleParse}>解析预览</Button>
          <Button
            type="primary"
            disabled={selectedRules.length === 0}
            loading={importing}
            onClick={handleImport}
          >
            导入 {selectedRules.length > 0 ? `${selectedRules.length} 条规则` : '选中规则'}
          </Button>
        </Space>
      }
      styles={{ body: { paddingTop: 8, maxHeight: '68vh', overflow: 'auto' } }}
    >
      <Alert
        type="info"
        showIcon
        style={{ marginBottom: 12 }}
        message={
          <Text style={{ fontSize: 12 }}>
            支持 OpenAPI 3.x / Swagger 2.0 的 <Text code>JSON</Text> 文档；解析 paths 下全部接口生成
            匹配规则（路径参数 <Text code>{'{}'}</Text> → <Text code>:</Text>，响应取首个 2xx 并按 schema
            （含 <Text code>$ref</Text>）生成响应体）。解析后<Text strong>按文档 tags 分组</Text>，勾选要导入的分组或行。
            规则会<Text strong>追加</Text>到当前服务。
          </Text>
        }
      />
      <Space direction="vertical" size={8} style={{ width: '100%' }}>
        <Space wrap>
          <Button size="small" icon={<UploadOutlined />} onClick={() => fileInputRef.current?.click()}>
            选择 .json 文件
          </Button>
          <input
            ref={fileInputRef}
            type="file"
            accept=".json,application/json"
            style={{ display: 'none' }}
            onChange={(e) => {
              const f = e.target.files?.[0];
              if (f) {
                const reader = new FileReader();
                reader.onload = () => {
                  setText(String(reader.result ?? ''));
                  setParsed(null);
                  setSelectedIds(new Set());
                  setError(null);
                  message.info(`已读取 ${f.name}，点击「解析预览」生成规则`);
                };
                reader.readAsText(f);
              }
              e.target.value = '';
            }}
          />
          <Text type="secondary" style={{ fontSize: 12 }}>或粘贴 JSON 文本：</Text>
        </Space>
        <textarea
          value={text}
          onChange={(e) => { setText(e.target.value); setParsed(null); setSelectedIds(new Set()); setError(null); }}
          placeholder='{"openapi":"3.0.3","paths":{"/users":{"get":{"responses":{"200":{"content":{"application/json":{"schema":{"type":"array","items":{"type":"object"}}}}}}}}}}'
          style={{
            width: '100%',
            minHeight: 110,
            fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Consolas, monospace',
            fontSize: 12,
            border: '1px solid #d9d9d9',
            borderRadius: 6,
            padding: 8,
            resize: 'vertical',
          }}
        />
        {error && <Alert type="error" showIcon message={<Text style={{ fontSize: 12 }}>{error}</Text>} />}
        {parsed && parsed.length > 0 && (
          <div style={{ marginTop: 4 }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 6 }}>
              <Checkbox
                checked={selectedIds.size === parsed.length && parsed.length > 0}
                indeterminate={selectedIds.size > 0 && selectedIds.size < parsed.length}
                onChange={(e) =>
                  setSelectedIds(e.target.checked ? new Set(parsed.map((r) => rowKey(r))) : new Set())
                }
              >
                全选
              </Checkbox>
              <Text type="secondary" style={{ fontSize: 12 }}>
                已选 {selectedIds.size} / {parsed.length} 条
              </Text>
            </div>
            {groups.map((g) => (
              <div key={g.group} style={{ marginBottom: 8 }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 4 }}>
                  <Checkbox
                    checked={groupChecked(g.rules)}
                    indeterminate={groupIndeterminate(g.rules)}
                    onChange={(e) => toggleGroup(g.rules, e.target.checked)}
                  />
                  <FolderOutlined style={{ fontSize: 12, opacity: 0.6 }} />
                  <Text strong style={{ fontSize: 12 }}>{g.group}</Text>
                  <Text type="secondary" style={{ fontSize: 11 }}>
                    {g.rules.filter((r) => selectedIds.has(rowKey(r))).length}/{g.rules.length}
                  </Text>
                </div>
                <Table
                  size="small"
                  rowKey={rowKey}
                  columns={columns}
                  dataSource={g.rules}
                  rowSelection={rowSelection}
                  pagination={false}
                />
              </div>
            ))}
          </div>
        )}
      </Space>
    </Modal>
  );
}

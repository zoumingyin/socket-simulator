import { useRef, useState } from 'react';
import { Alert, Button, Modal, Space, Table, Tag, Typography, message } from 'antd';
import { UploadOutlined } from '@ant-design/icons';
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
 * 解析后预览规则列表，确认后一次性追加到当前服务的匹配规则。
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
  const [parsing, setParsing] = useState(false);
  const [importing, setImporting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const reset = () => {
    setText('');
    setParsed(null);
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
      message.success(`解析成功：提取 ${rules.length} 条接口`);
    } catch (e) {
      setParsed(null);
      setError((e as Error).message);
    } finally {
      setParsing(false);
    }
  };

  const handleImport = async () => {
    if (!parsed || parsed.length === 0) return;
    setImporting(true);
    try {
      await onImport(parsed);
      message.success(`已导入 ${parsed.length} 条规则`);
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
      width: 90,
      render: (m: string) => <Tag color={METHOD_COLOR[m] ?? 'default'}>{m}</Tag>,
    },
    { title: '路径', dataIndex: 'pathPattern', ellipsis: true },
    { title: '状态码', dataIndex: 'responseStatusCode', width: 80 },
    { title: '规则名', dataIndex: 'name', ellipsis: true },
  ];

  return (
    <Modal
      title="导入 Swagger / OpenAPI 文档"
      open={open}
      onCancel={handleClose}
      width={720}
      centered
      footer={
        <Space>
          <Button onClick={handleClose}>取消</Button>
          <Button loading={parsing} onClick={handleParse}>解析预览</Button>
          <Button
            type="primary"
            disabled={!parsed || parsed.length === 0}
            loading={importing}
            onClick={handleImport}
          >
            导入 {parsed ? `${parsed.length} 条规则` : ''}
          </Button>
        </Space>
      }
      styles={{ body: { paddingTop: 8, maxHeight: '65vh', overflow: 'auto' } }}
    >
      <Alert
        type="info"
        showIcon
        style={{ marginBottom: 12 }}
        message={
          <Text style={{ fontSize: 12 }}>
            支持 OpenAPI 3.x / Swagger 2.0 的 <Text code>JSON</Text> 文档；解析 paths 下全部接口生成
            匹配规则（路径参数 <Text code>{'{}'}</Text> → <Text code>:</Text>，响应取首个 2xx 与 schema 示例）。
            规则会<Text strong>追加</Text>到当前服务，可在导入后逐条编辑。
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
          onChange={(e) => { setText(e.target.value); setParsed(null); setError(null); }}
          placeholder='{"openapi":"3.0.3","paths":{"/users":{"get":{"responses":{"200":{"content":{"application/json":{"schema":{"type":"array","items":{"type":"object"}}}}}}}}}}'
          style={{
            width: '100%',
            minHeight: 120,
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
          <Table
            size="small"
            rowKey={(r) => r.id ?? `${r.method}-${r.pathPattern}`}
            columns={columns}
            dataSource={parsed}
            pagination={false}
            scroll={{ y: 260 }}
            style={{ marginTop: 4 }}
          />
        )}
      </Space>
    </Modal>
  );
}

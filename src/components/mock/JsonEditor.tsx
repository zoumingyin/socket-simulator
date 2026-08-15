import { useMemo, useState } from 'react';
import { Segmented, Select, Tooltip, Button, Tag, Input, message } from 'antd';
import { CopyOutlined, ClearOutlined, DeleteOutlined, PlusOutlined } from '@ant-design/icons';
import { JSON_TEMPLATES, type JsonValueType } from './constants.js';

const { Option } = Select;

export function JsonEditor({ value, onChange, rows = 6, placeholder }: {
  value?: string;
  onChange?: (v: string) => void;
  rows?: number;
  placeholder?: string;
}) {
  const [mode, setMode] = useState<'text' | 'kv'>('text');
  const [kvPairs, setKvPairs] = useState<Array<{ key: string; value: string; type: JsonValueType }>>([]);
  const [tplKey, setTplKey] = useState<string | undefined>(undefined);

  const current = value ?? '';

  const jsonValid = useMemo(() => {
    if (!current) return true;
    try { JSON.parse(current); return true; } catch { return false; }
  }, [current]);

  const formatJson = () => {
    try {
      onChange?.(JSON.stringify(JSON.parse(current), null, 2));
      message.success('已格式化');
    } catch { message.warning('JSON 无效，无法格式化'); }
  };

  const minifyJson = () => {
    try {
      onChange?.(JSON.stringify(JSON.parse(current)));
      message.success('已压缩');
    } catch { message.warning('JSON 无效，无法压缩'); }
  };

  const copyJson = async () => {
    try {
      await navigator.clipboard.writeText(current);
      message.success('已复制到剪贴板');
    } catch { message.warning('复制失败'); }
  };

  const clearJson = () => { onChange?.(''); };

  const applyTemplate = (v: string) => {
    onChange?.(v);
    message.success('已插入模板');
  };

  const syncKvToText = (pairs: Array<{ key: string; value: string; type: JsonValueType }>) => {
    const obj: Record<string, unknown> = {};
    pairs.forEach((p) => {
      if (!p.key) return;
      switch (p.type) {
        case 'number': obj[p.key] = Number(p.value) || 0; break;
        case 'boolean': obj[p.key] = p.value === 'true'; break;
        case 'null': obj[p.key] = null; break;
        default: obj[p.key] = p.value;
      }
    });
    onChange?.(JSON.stringify(obj, null, 2));
  };

  const switchToKv = () => {
    try {
      const obj = JSON.parse(current || '{}');
      if (typeof obj !== 'object' || obj === null || Array.isArray(obj)) {
        message.warning('仅支持 JSON 对象类型转换为键值编辑');
        return;
      }
      const pairs = Object.entries(obj).map(([k, v]) => {
        if (v === null) return { key: k, value: '', type: 'null' as JsonValueType };
        if (typeof v === 'boolean') return { key: k, value: String(v), type: 'boolean' as JsonValueType };
        if (typeof v === 'number') return { key: k, value: String(v), type: 'number' as JsonValueType };
        return { key: k, value: String(v), type: 'string' as JsonValueType };
      });
      setKvPairs(pairs.length > 0 ? pairs : [{ key: '', value: '', type: 'string' }]);
      setMode('kv');
    } catch {
      message.warning('JSON 无效，无法转换为键值编辑');
    }
  };

  const kvAdd = () => {
    const next = [...kvPairs, { key: '', value: '', type: 'string' as JsonValueType }];
    setKvPairs(next);
    syncKvToText(next);
  };
  const kvRemove = (i: number) => {
    const next = kvPairs.filter((_, idx) => idx !== i);
    setKvPairs(next);
    syncKvToText(next);
  };
  const kvUpdate = (i: number, patch: Partial<{ key: string; value: string; type: JsonValueType }>) => {
    const next = kvPairs.map((p, idx) => idx === i ? { ...p, ...patch } : p);
    setKvPairs(next);
    syncKvToText(next);
  };

  return (
    <div style={{ width: '100%', maxWidth: '100%', minWidth: 0, overflow: 'hidden' }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 4, marginBottom: 6, flexWrap: 'wrap' }}>
        <Segmented
          size="small"
          value={mode}
          onChange={(v) => (v === 'kv' ? switchToKv() : setMode('text'))}
          options={[
            { label: '文本', value: 'text' },
            { label: '键值', value: 'kv' },
          ]}
        />
        {mode === 'text' && (
          <>
            <Select
              size="small"
              placeholder="插入模板"
              style={{ width: 130 }}
              value={tplKey}
              onChange={(v) => { if (v) { applyTemplate(v); setTplKey(undefined); } }}
              allowClear
              options={JSON_TEMPLATES.map((t) => ({ label: t.label, value: t.value }))}
            />
            <Tooltip title="格式化"><Button size="small" type="text" onClick={formatJson}>格式化</Button></Tooltip>
            <Tooltip title="压缩"><Button size="small" type="text" onClick={minifyJson}>压缩</Button></Tooltip>
          </>
        )}
        {mode === 'kv' && <Button size="small" type="text" onClick={() => setMode('text')}>返回文本</Button>}
        <div style={{ marginLeft: 'auto', display: 'flex', gap: 2 }}>
          <Tooltip title="复制"><Button size="small" type="text" icon={<CopyOutlined />} onClick={copyJson} /></Tooltip>
          <Tooltip title="清空"><Button size="small" type="text" icon={<ClearOutlined />} onClick={clearJson} /></Tooltip>
          <Tag color={jsonValid ? 'green' : 'red'} style={{ fontSize: 11, margin: 0 }}>
            {jsonValid ? 'JSON 有效' : 'JSON 无效'}
          </Tag>
        </div>
      </div>

      {mode === 'text' && (
        <Input.TextArea
          rows={rows}
          value={current}
          onChange={(e) => onChange?.(e.target.value)}
          placeholder={placeholder || '{"key":"value"}'}
          style={{ fontFamily: 'monospace', borderColor: jsonValid ? undefined : '#ff4d4f' }}
        />
      )}

      {mode === 'kv' && (
        <div style={{ border: '1px solid #d9d9d9', borderRadius: 6, padding: 8, maxWidth: '100%', overflow: 'hidden' }}>
          {kvPairs.map((p, i) => (
            <div key={i} style={{ display: 'flex', gap: 4, marginBottom: 4, alignItems: 'center', minWidth: 0 }}>
              <Input size="small" placeholder="字段名" value={p.key} onChange={(e) => kvUpdate(i, { key: e.target.value })} style={{ width: 100, flexShrink: 0 }} />
              <Select size="small" value={p.type} onChange={(v: JsonValueType) => kvUpdate(i, { type: v })} style={{ width: 88, flexShrink: 0 }}>
                <Option value="string">string</Option>
                <Option value="number">number</Option>
                <Option value="boolean">boolean</Option>
                <Option value="null">null</Option>
              </Select>
              {p.type === 'boolean' ? (
                <Select size="small" value={p.value === 'true' ? 'true' : 'false'} onChange={(v) => kvUpdate(i, { value: v })} style={{ width: 88, flexShrink: 0 }}>
                  <Option value="true">true</Option>
                  <Option value="false">false</Option>
                </Select>
              ) : p.type === 'null' ? (
                <Input size="small" value="null" disabled style={{ flex: 1, minWidth: 0 }} />
              ) : (
                <Input size="small" placeholder="值" value={p.value} onChange={(e) => kvUpdate(i, { value: e.target.value })} style={{ flex: 1, minWidth: 0 }} />
              )}
              <Button danger size="small" icon={<DeleteOutlined />} onClick={() => kvRemove(i)} style={{ flexShrink: 0 }} />
            </div>
          ))}
          <Button size="small" type="dashed" icon={<PlusOutlined />} onClick={kvAdd} style={{ marginTop: 4 }}>添加字段</Button>
        </div>
      )}
    </div>
  );
}

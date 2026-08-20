import type { CSSProperties } from 'react';
import { AutoComplete } from 'antd';
import { HTTP_STATUS_GROUPS } from './constants.js';

/**
 * 状态码选择框：可自由输入任意状态码（AutoComplete），选项按
 * 1xx/2xx/3xx/4xx/5xx 分组展示（同 Swagger UI 风格），支持数字或语义关键字搜索。
 */
export function StatusCodeSelect({
  value,
  onChange,
  style,
}: {
  value?: number;
  onChange?: (v: number) => void;
  style?: CSSProperties;
}) {
  const options = HTTP_STATUS_GROUPS.map((g) => ({
    label: g.label,
    options: g.codes.map((c) => ({ value: String(c.value), label: c.label })),
  }));

  const toNumber = (v: string): number => {
    const n = Number(v);
    return Number.isFinite(n) ? n : 200;
  };

  return (
    <AutoComplete
      value={value === undefined ? undefined : String(value)}
      onChange={(v) => onChange?.(toNumber(v))}
      options={options}
      style={style}
      placeholder="选择或输入状态码"
      filterOption={(input, option) => {
        const o = option as { value?: string; label?: unknown };
        const text = `${o.value ?? ''} ${String(o.label ?? '')}`.toLowerCase();
        return text.includes(input.toLowerCase());
      }}
    />
  );
}

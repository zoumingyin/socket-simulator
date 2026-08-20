import type { CSSProperties } from 'react';
import { Select } from 'antd';
import { HTTP_STATUS_CODES } from './constants.js';

/**
 * 状态码下拉框：常用 HTTP 状态码（200/201/404/500 …），支持数字或语义关键字搜索。
 * 若需非常用状态码，可保持数字输入场景改用 InputNumber（本组件聚焦常用选择）。
 */
export function StatusCodeSelect({
  value,
  onChange,
  style,
  allowCustom,
}: {
  value?: number;
  onChange?: (v: number) => void;
  style?: CSSProperties;
  /** 允许输入非常用状态码（tags 模式，单值） */
  allowCustom?: boolean;
}) {
  const opts = HTTP_STATUS_CODES.map((c) => ({
    value: c.value,
    label: c.label,
  }));
  if (allowCustom) {
    return (
      <Select
        mode="tags"
        maxCount={1}
        showSearch
        value={value === undefined ? undefined : [value]}
        onChange={(arr: (number | string)[]) => {
          const raw = arr[arr.length - 1];
          const n = typeof raw === 'number' ? raw : Number(raw);
          onChange?.(Number.isFinite(n) ? n : 200);
        }}
        options={opts}
        style={style}
        placeholder="选择或输入状态码"
        optionFilterProp="label"
        tagRender={(p) => <span>{p.label}</span>}
      />
    );
  }
  return (
    <Select
      showSearch
      value={value}
      onChange={onChange}
      options={opts}
      style={style}
      placeholder="选择状态码"
      optionFilterProp="label"
    />
  );
}

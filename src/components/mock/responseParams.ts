/**
 * responseParams.ts — 响应参数 ↔ 响应体 JSON 双向转换（纯函数，可单测）
 *
 * 响应参数表把「手写响应体 JSON」变为结构化声明：
 * - key：字段路径（`a.b.c` 表示嵌套；顶层字段直接写名）
 * - type：string / number / boolean / object / array / null
 * - value：示例值（string 为字面值；number/boolean 为原文；array 为 JSON 文本）
 *
 * 约束（MVP）：
 * - 嵌套 object 递归平铺为「点路径」行；空 object 作为单行（type=object）
 * - array 作为单行（value 存 JSON 文本，如 `["a","b"]`），不展开内部
 * - 顶层数组响应体支持（单行 key 为空 + type=array）
 */

export type RespType = 'string' | 'number' | 'boolean' | 'object' | 'array' | 'null';

export interface RespParam {
  key: string;
  type: RespType;
  value?: string;
}

const RESP_TYPES: RespType[] = ['string', 'number', 'boolean', 'object', 'array', 'null'];

export function isRespType(t: string): t is RespType {
  return (RESP_TYPES as string[]).includes(t);
}

/** 响应体 JSON → 参数表；JSON 非法返回 [] */
export function parseBodyToParams(body: string): RespParam[] {
  let obj: unknown;
  try {
    obj = JSON.parse(body);
  } catch {
    return [];
  }
  const out: RespParam[] = [];
  walk(obj, '', out);
  return out;
}

function walk(obj: unknown, prefix: string, out: RespParam[]) {
  if (obj === null) {
    out.push({ key: prefix, type: 'null' });
    return;
  }
  if (Array.isArray(obj)) {
    out.push({ key: prefix, type: 'array', value: JSON.stringify(obj) });
    return;
  }
  if (typeof obj === 'object') {
    const entries = Object.entries(obj as Record<string, unknown>);
    if (entries.length === 0) {
      out.push({ key: prefix, type: 'object' });
      return;
    }
    for (const [k, v] of entries) {
      walk(v, prefix ? `${prefix}.${k}` : k, out);
    }
    return;
  }
  const t = typeof obj;
  out.push({
    key: prefix,
    type: t === 'number' ? 'number' : t === 'boolean' ? 'boolean' : 'string',
    value: String(obj),
  });
}

/** 参数表 → 响应体 JSON（pretty）；参数为空返回 '{}' */
export function paramsToBody(params: RespParam[]): string {
  // 顶层数组响应体：唯一一行且 key 为空（数组行），须保留不被过滤
  const topLevelArray = params.length === 1 && params[0].key.trim() === '';
  const valid = topLevelArray ? params : params.filter((p) => p.key.trim() !== '');
  if (valid.length === 0) return '{}';

  if (topLevelArray) {
    return JSON.stringify(buildValue(valid[0]), null, 2);
  }

  const root: Record<string, unknown> = {};
  for (const p of valid) {
    const segs = p.key.split('.').map((s) => s.trim());
    let cur: Record<string, unknown> = root;
    for (let i = 0; i < segs.length - 1; i++) {
      const s = segs[i];
      if (!s) continue;
      const next = cur[s];
      if (typeof next !== 'object' || next === null || Array.isArray(next)) {
        cur[s] = {};
      }
      cur = cur[s] as Record<string, unknown>;
    }
    const last = segs[segs.length - 1];
    if (last) cur[last] = buildValue(p);
  }
  return JSON.stringify(root, null, 2);
}

function buildValue(p: RespParam): unknown {
  switch (p.type) {
    case 'string':
      return p.value ?? '';
    case 'number': {
      const n = Number(p.value);
      return Number.isFinite(n) ? n : 0;
    }
    case 'boolean':
      return p.value === 'true' || p.value === '1' || p.value === '是';
    case 'object':
      return {};
    case 'array': {
      try {
        const v = JSON.parse(p.value ?? '');
        return Array.isArray(v) ? v : [];
      } catch {
        return [];
      }
    }
    case 'null':
      return null;
    default:
      return null;
  }
}

/** 新增空参数行 */
export function emptyParam(): RespParam {
  return { key: '', type: 'string', value: '' };
}

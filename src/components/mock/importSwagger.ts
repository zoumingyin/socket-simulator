/**
 * importSwagger.ts — 解析 Swagger/OpenAPI 文档为 Mock 规则（纯函数，可单测）
 *
 * 支持：
 * - OpenAPI 3.0 / 3.1（paths[path][method]，responses[].content['application/json']）
 * - Swagger 2.0（paths[path][method]，responses[].schema）
 * - 输入为 JSON 文本（YAML 暂不支持，前端提示）
 *
 * 转换规则：
 * - path 参数 `{id}` → mock 规则路径参数 `:id`
 * - 响应状态码：优先首个 2xx（含 `2XX` 通配），否则首个 3 位数字，兜底 200
 * - 响应体：取 response 的 `example` → `schema`（递归生成示例值）→ 兜底 `{}`
 * - 规则名：operationId → summary → `{METHOD} {path}`
 */

import type { HttpMethod, MockRule } from '../../types/index.js';

const HTTP_METHODS = ['get', 'post', 'put', 'delete', 'patch', 'head', 'options'] as const;

/** 解析 Swagger/OpenAPI JSON 文本 → MockRule[]；格式非法时抛中文错误 */
export function parseSwaggerToRules(specText: string): MockRule[] {
  let spec: unknown;
  try {
    spec = JSON.parse(specText);
  } catch {
    throw new Error('JSON 解析失败：请确认是合法 JSON（OpenAPI 3.x 或 Swagger 2.0 均可，YAML 暂不支持）');
  }
  if (!spec || typeof spec !== 'object' || Array.isArray(spec)) {
    throw new Error('文档格式不正确：应为 JSON 对象（{ openapi / swagger, paths }）');
  }
  const paths = (spec as Record<string, unknown>).paths;
  if (!paths || typeof paths !== 'object' || Array.isArray(paths)) {
    throw new Error('文档缺少 paths 节点：无法提取接口定义');
  }

  const rules: MockRule[] = [];
  for (const [path, pathItem] of Object.entries(paths as Record<string, unknown>)) {
    if (!pathItem || typeof pathItem !== 'object' || Array.isArray(pathItem)) continue;
    const item = pathItem as Record<string, unknown>;
    for (const m of HTTP_METHODS) {
      const op = item[m];
      if (!op || typeof op !== 'object' || Array.isArray(op)) continue;
      rules.push(operationToRule(m.toUpperCase() as HttpMethod, path, op as OperationLike));
    }
  }

  if (rules.length === 0) {
    throw new Error('未从文档中解析到任何接口：请检查 paths 下是否包含 HTTP 方法（get/post/put/delete/patch/head/options）');
  }
  return rules;
}

/** OpenAPI operation 的子集（只取需要的字段） */
interface OperationLike {
  operationId?: unknown;
  summary?: unknown;
  responses?: unknown;
}

function operationToRule(method: HttpMethod, path: string, op: OperationLike): MockRule {
  const pathPattern = path.replace(/\{([^}]+)\}/g, ':$1');
  const { status, body } = pickResponse(op.responses);
  const summary = typeof op.summary === 'string' ? op.summary : undefined;
  const name =
    (typeof op.operationId === 'string' && op.operationId) ||
    summary ||
    `${method} ${path}`;
  return {
    id: `rule_${Date.now()}_${Math.random().toString(36).slice(2, 10)}`,
    name,
    method,
    pathPattern,
    responseStatusCode: status,
    responseBody: body,
    responseDelayMs: 0,
    enabled: true,
    matchHeaders: [],
    matchQuery: [],
    responseHeaders: [],
  };
}

function pickResponse(responses: unknown): { status: number; body: string } {
  if (!responses || typeof responses !== 'object') return { status: 200, body: '{}' };
  const map = responses as Record<string, unknown>;
  const keys = Object.keys(map);
  const statusKey =
    keys.find((k) => /^2\d\d$/.test(k)) ||
    keys.find((k) => k === '2XX') ||
    keys.find((k) => /^[1-5]\d\d$/.test(k)) ||
    keys[0] ||
    '200';
  const status = /^\d{3}$/.test(statusKey) ? Number(statusKey) : 200;
  const resp = map[statusKey];
  if (!resp || typeof resp !== 'object') return { status, body: '{}' };

  const r = resp as Record<string, unknown>;
  let body = '{}';
  // OpenAPI 3.x：responses[].content['application/json']
  const content = r.content as Record<string, unknown> | undefined;
  if (content && typeof content === 'object') {
    const json = content['application/json'] as Record<string, unknown> | undefined;
    if (json && typeof json === 'object') {
      body = bodyFromExampleOrSchema(json);
    }
  }
  // Swagger 2.0：responses[].schema
  if (body === '{}' && r.schema && typeof r.schema === 'object') {
    body = JSON.stringify(schemaToExample(r.schema as Record<string, unknown>), null, 2);
  }
  return { status, body };
}

function bodyFromExampleOrSchema(json: Record<string, unknown>): string {
  const example = (json as Record<string, unknown>).example;
  if (example !== undefined) {
    return typeof example === 'string' ? example : JSON.stringify(example, null, 2);
  }
  const schema = json.schema as Record<string, unknown> | undefined;
  if (schema && typeof schema === 'object') {
    return JSON.stringify(schemaToExample(schema), null, 2);
  }
  return '{}';
}

/** 依据 JSON Schema 递归生成示例值（供 mock 响应体使用） */
export function schemaToExample(schema: Record<string, unknown>): unknown {
  if (!schema || typeof schema !== 'object') return null;
  const s = schema as Record<string, unknown>;
  if (Array.isArray(s.enum) && (s.enum as unknown[]).length > 0) return (s.enum as unknown[])[0];
  if (s.example !== undefined) return s.example;
  if (s.default !== undefined) return s.default;
  if (Array.isArray(s.oneOf) && (s.oneOf as unknown[]).length > 0) {
    return schemaToExample((s.oneOf as unknown[])[0] as Record<string, unknown>);
  }
  if (Array.isArray(s.anyOf) && (s.anyOf as unknown[]).length > 0) {
    return schemaToExample((s.anyOf as unknown[])[0] as Record<string, unknown>);
  }
  if (Array.isArray(s.allOf)) {
    const merged: Record<string, unknown> = {};
    for (const sub of s.allOf as unknown[]) {
      if (sub && typeof sub === 'object') {
        Object.assign(merged, schemaToExample(sub as Record<string, unknown>));
      }
    }
    return merged;
  }
  const type = Array.isArray(s.type) ? (s.type as unknown[])[0] : s.type;
  const format = typeof s.format === 'string' ? s.format : undefined;
  switch (type) {
    case 'string': {
      switch (format) {
        case 'date-time': return '2026-01-01T00:00:00Z';
        case 'date': return '2026-01-01';
        case 'uuid': return '00000000-0000-0000-0000-000000000000';
        case 'email': return 'user@example.com';
        default: return 'string';
      }
    }
    case 'number':
    case 'integer':
      return 0;
    case 'boolean':
      return false;
    case 'array':
      return s.items && typeof s.items === 'object'
        ? [schemaToExample(s.items as Record<string, unknown>)]
        : [];
    case 'object':
    default: {
      const obj: Record<string, unknown> = {};
      if (s.properties && typeof s.properties === 'object') {
        for (const [k, v] of Object.entries(s.properties as Record<string, unknown>)) {
          if (v && typeof v === 'object') {
            obj[k] = schemaToExample(v as Record<string, unknown>);
          }
        }
      }
      return obj;
    }
  }
}

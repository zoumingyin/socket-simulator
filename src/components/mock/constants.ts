import type { HttpMethod, MatchKind } from '../../types/index.js';

export const MOCK_METHODS: HttpMethod[] = ['GET', 'POST', 'PUT', 'DELETE', 'PATCH', 'HEAD', 'OPTIONS', 'ANY'];
export const TEST_METHODS: HttpMethod[] = ['GET', 'POST', 'PUT', 'DELETE', 'PATCH', 'HEAD', 'OPTIONS'];
export const MATCH_KINDS: MatchKind[] = ['exact', 'contains', 'regex', 'exists'];

/** 常用 HTTP 状态码（按 1xx/2xx/3xx/4xx/5xx 分组，同 Swagger UI 风格） */
export const HTTP_STATUS_GROUPS: Array<{ label: string; codes: Array<{ value: number; label: string }> }> = [
  {
    label: '1xx · 信息',
    codes: [
      { value: 100, label: '100 Continue' },
      { value: 101, label: '101 Switching Protocols' },
    ],
  },
  {
    label: '2xx · 成功',
    codes: [
      { value: 200, label: '200 OK' },
      { value: 201, label: '201 Created' },
      { value: 202, label: '202 Accepted' },
      { value: 204, label: '204 No Content' },
    ],
  },
  {
    label: '3xx · 重定向',
    codes: [
      { value: 301, label: '301 Moved Permanently' },
      { value: 302, label: '302 Found' },
      { value: 304, label: '304 Not Modified' },
    ],
  },
  {
    label: '4xx · 客户端错误',
    codes: [
      { value: 400, label: '400 Bad Request' },
      { value: 401, label: '401 Unauthorized' },
      { value: 403, label: '403 Forbidden' },
      { value: 404, label: '404 Not Found' },
      { value: 405, label: '405 Method Not Allowed' },
      { value: 409, label: '409 Conflict' },
      { value: 422, label: '422 Unprocessable Entity' },
      { value: 429, label: '429 Too Many Requests' },
    ],
  },
  {
    label: '5xx · 服务器错误',
    codes: [
      { value: 500, label: '500 Internal Server Error' },
      { value: 502, label: '502 Bad Gateway' },
      { value: 503, label: '503 Service Unavailable' },
      { value: 504, label: '504 Gateway Timeout' },
    ],
  },
];

/** 常用 HTTP 状态码（扁平列表，兼容旧引用） */
export const HTTP_STATUS_CODES: Array<{ value: number; label: string }> =
  HTTP_STATUS_GROUPS.flatMap((g) => g.codes);

export const JSON_TEMPLATES: Array<{ key: string; label: string; value: string }> = [
  {
    key: 'success',
    label: '成功响应',
    value: JSON.stringify({ success: true, data: {}, message: 'ok' }, null, 2),
  },
  {
    key: 'error',
    label: '错误响应',
    value: JSON.stringify({ success: false, error: { code: 'ERROR_CODE', message: '错误描述' } }, null, 2),
  },
  {
    key: 'paginated',
    label: '分页列表',
    value: JSON.stringify({ success: true, data: { list: [], total: 0, page: 1, pageSize: 20 } }, null, 2),
  },
  { key: 'array', label: '空数组', value: '[]' },
  { key: 'object', label: '空对象', value: '{}' },
  {
    key: 'user',
    label: '用户示例',
    value: JSON.stringify({ id: 1, name: '张三', email: 'zhangsan@example.com', role: 'admin', createdAt: '2026-01-01T00:00:00Z' }, null, 2),
  },
  {
    key: 'token',
    label: 'Token 响应',
    value: JSON.stringify({ token: 'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...', expiresIn: 3600, refreshToken: 'refresh_token_here' }, null, 2),
  },
];

export type JsonValueType = 'string' | 'number' | 'boolean' | 'null';

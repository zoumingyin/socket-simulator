import type { HttpMethod, MatchKind } from '../../types/index.js';

export const MOCK_METHODS: HttpMethod[] = ['GET', 'POST', 'PUT', 'DELETE', 'PATCH', 'HEAD', 'OPTIONS', 'ANY'];
export const TEST_METHODS: HttpMethod[] = ['GET', 'POST', 'PUT', 'DELETE', 'PATCH', 'HEAD', 'OPTIONS'];
export const MATCH_KINDS: MatchKind[] = ['exact', 'contains', 'regex', 'exists'];

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

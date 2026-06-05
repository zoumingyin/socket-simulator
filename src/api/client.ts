/**
 * api/client.ts - 前端 API 调用层
 * 封装所有 REST API 调用，自动拼接 base URL
 */
const API_BASE = 'http://localhost:3080';

async function apiFetch<T = unknown>(
  path: string,
  options: RequestInit = {}
): Promise<{ success: boolean; data?: T; error?: string }> {
  const url = path.startsWith('http') ? path : `${API_BASE}${path}`;
  try {
    const res = await fetch(url, {
      ...options,
      headers: {
        'Content-Type': 'application/json',
        ...options.headers,
      },
    });
    const json = await res.json();
    return json as { success: boolean; data?: T; error?: string };
  } catch (err) {
    return { success: false, error: (err as Error).message };
  }
}

export { apiFetch };

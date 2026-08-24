import type { AdminOverview, AdminPurgeResult, AdminSystemLockStatus } from './adminTypes'

export class AdminApiError extends Error {
  constructor(public readonly status: number) {
    super(status === 401 ? '登录已过期' : status === 403 ? '当前账户没有系统管理权限' : `后台接口返回 ${status}`)
  }
}

async function adminRequest(path: string, token: string, method = 'GET', body?: unknown): Promise<Response> {
  const response = await fetch(path, {
    method,
    cache: 'no-store',
    headers: {
      Accept: 'application/json',
      Authorization: `Bearer ${token}`,
      ...(body ? { 'Content-Type': 'application/json' } : {}),
    },
    body: body ? JSON.stringify(body) : undefined,
  })
  if (!response.ok) throw new AdminApiError(response.status)
  return response
}

export async function getAdminOverview(token: string): Promise<AdminOverview> {
  return (await adminRequest('/api/admin/overview', token)).json() as Promise<AdminOverview>
}

export async function purgeAdminRetention(token: string): Promise<AdminPurgeResult> {
  return (await adminRequest('/api/admin/maintenance/purge', token, 'POST')).json() as Promise<AdminPurgeResult>
}

export async function setAdminChatLock(locked: boolean, token: string): Promise<AdminSystemLockStatus> {
  return (await adminRequest('/api/admin/chat-lock', token, 'PUT', { locked })).json() as Promise<AdminSystemLockStatus>
}

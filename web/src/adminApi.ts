import type { AdminOverview, AdminPurgeResult } from './adminTypes'

export class AdminApiError extends Error {
  constructor(public readonly status: number) {
    super(status === 401 ? '登录已过期' : status === 403 ? '当前账户没有系统管理权限' : `后台接口返回 ${status}`)
  }
}

async function adminRequest(path: string, token: string, method = 'GET'): Promise<Response> {
  const response = await fetch(path, {
    method,
    cache: 'no-store',
    headers: {
      Accept: 'application/json',
      Authorization: `Bearer ${token}`,
    },
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

import { authHeaders, request } from './api'

export interface DeviceSession {
  id: string
  device_name: string
  ip_hint: string | null
  created_at: string
  last_used_at: string
  expires_at: string
  current: boolean
}

async function sessionRequest(path: string, token: string, options: RequestInit = {}): Promise<Response> {
  const response = await request(path, {
    ...options,
    headers: { ...authHeaders(token), ...options.headers },
  })
  if (response.status === 401) throw new Error('登录已过期')
  if (response.status === 404) throw new Error('设备登录已不存在')
  if (response.status === 409) throw new Error('当前设备请使用退出登录')
  if (!response.ok) throw new Error(`设备登录请求失败：${response.status}`)
  return response
}

export async function listDeviceSessions(token: string): Promise<DeviceSession[]> {
  const response = await sessionRequest('/api/users/me/sessions', token)
  return response.json() as Promise<DeviceSession[]>
}

export async function revokeDeviceSession(token: string, id: string): Promise<void> {
  await sessionRequest(`/api/users/me/sessions/${encodeURIComponent(id)}`, token, { method: 'DELETE' })
}

export async function revokeOtherDeviceSessions(token: string): Promise<void> {
  await sessionRequest('/api/users/me/sessions/others', token, { method: 'DELETE' })
}

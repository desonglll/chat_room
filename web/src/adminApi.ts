import type {
  AdminOverview,
  AdminPurgeResult,
  AdminRoomLockStatus,
  AdminSystemLockStatus,
  AdminVectorProbeResult,
  AdminAiModelOption,
  SaveAdminAiModelOption,
} from './adminTypes'

export class AdminApiError extends Error {
  constructor(public readonly status: number) {
    super(status === 401 ? '登录已过期' : status === 403 ? '当前账户没有系统管理权限' : `后台接口返回 ${status}`)
  }
}

export async function listAdminAiModels(token: string): Promise<AdminAiModelOption[]> {
  return (await adminRequest('/api/admin/ai-models', token)).json() as Promise<AdminAiModelOption[]>
}

export async function createAdminAiModel(
  token: string,
  payload: SaveAdminAiModelOption,
): Promise<AdminAiModelOption> {
  return (await adminRequest('/api/admin/ai-models', token, 'POST', payload)).json() as Promise<AdminAiModelOption>
}

export async function updateAdminAiModel(
  token: string,
  id: string,
  payload: SaveAdminAiModelOption,
): Promise<AdminAiModelOption> {
  return (
    await adminRequest(`/api/admin/ai-models/${encodeURIComponent(id)}`, token, 'PUT', payload)
  ).json() as Promise<AdminAiModelOption>
}

export async function deleteAdminAiModel(token: string, id: string): Promise<void> {
  await adminRequest(`/api/admin/ai-models/${encodeURIComponent(id)}`, token, 'DELETE')
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

export async function probeAdminVectorSearch(
  roomId: string,
  query: string,
  token: string,
): Promise<AdminVectorProbeResult> {
  return (
    await adminRequest('/api/admin/vector/probe', token, 'POST', { room_id: roomId, query })
  ).json() as Promise<AdminVectorProbeResult>
}

export async function setAdminChatLock(locked: boolean, token: string): Promise<AdminSystemLockStatus> {
  return (await adminRequest('/api/admin/chat-lock', token, 'PUT', { locked })).json() as Promise<AdminSystemLockStatus>
}

export async function getAdminRoomLock(roomId: string, token: string): Promise<AdminRoomLockStatus> {
  return (await adminRequest(`/api/admin/room-locks/${roomId}`, token)).json() as Promise<AdminRoomLockStatus>
}

export async function setAdminRoomLock(roomId: string, locked: boolean, token: string): Promise<AdminRoomLockStatus> {
  return (
    await adminRequest(`/api/admin/room-locks/${roomId}`, token, 'PUT', { locked })
  ).json() as Promise<AdminRoomLockStatus>
}

import type {
  AdminOverview,
  AdminPurgeResult,
  AdminRoomLockStatus,
  AdminSystemLockStatus,
  AdminVectorProbeResult,
  AdminAiModelOption,
  AdminIndexSyncResult,
  AdminIndexSyncTarget,
  AdminRestoreBackupResult,
  AdminRestoreValidationResult,
  AdminBackupRun,
  AdminBackupStatus,
  SaveAdminAiModelOption,
} from './adminTypes'

export class AdminApiError extends Error {
  constructor(
    public readonly status: number,
    detail?: string,
  ) {
    super(
      detail ||
        (status === 401 ? '登录已过期' : status === 403 ? '当前账户没有系统管理权限' : `后台接口返回 ${status}`),
    )
  }
}

export async function listAdminAiModels(token: string): Promise<AdminAiModelOption[]> {
  return (await adminRequest('/api/admin/ai-models', token)).json() as Promise<AdminAiModelOption[]>
}

export async function createAdminAiModel(token: string, payload: SaveAdminAiModelOption): Promise<AdminAiModelOption> {
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
  if (!response.ok) await throwAdminError(response)
  return response
}

async function throwAdminError(response: Response): Promise<never> {
  let detail = ''
  try {
    const payload = (await response.json()) as { error?: unknown }
    if (typeof payload.error === 'string') detail = payload.error
  } catch {
    // Some existing endpoints intentionally return an empty error body.
  }
  throw new AdminApiError(response.status, detail)
}

export async function exportAdminBackup(
  token: string,
  includeFiles: boolean,
): Promise<{ blob: Blob; filename: string }> {
  const response = await adminRequest('/api/admin/backups/export', token, 'POST', { include_files: includeFiles })
  const disposition = response.headers.get('content-disposition') || ''
  const filename = disposition.match(/filename="([^"]+)"/)?.[1] || 'chat-room-backup.tar.gz'
  return { blob: await response.blob(), filename }
}

export async function getAdminBackupStatus(token: string): Promise<AdminBackupStatus> {
  return (await adminRequest('/api/admin/backups', token)).json() as Promise<AdminBackupStatus>
}

export async function runAdminBackup(token: string, includeFiles: boolean): Promise<AdminBackupRun> {
  return (
    await adminRequest('/api/admin/backups/run', token, 'POST', { include_files: includeFiles })
  ).json() as Promise<AdminBackupRun>
}

async function uploadBackup(path: string, token: string, file: File, confirmation?: string): Promise<Response> {
  const form = new FormData()
  form.append('file', file)
  if (confirmation) form.append('confirmation', confirmation)
  const response = await fetch(path, {
    method: 'POST',
    cache: 'no-store',
    headers: { Authorization: `Bearer ${token}` },
    body: form,
  })
  if (!response.ok) await throwAdminError(response)
  return response
}

export async function validateAdminBackup(token: string, file: File): Promise<AdminRestoreValidationResult> {
  return (await uploadBackup('/api/admin/backups/restore', token, file)).json() as Promise<AdminRestoreValidationResult>
}

export async function executeAdminBackup(token: string, file: File): Promise<AdminRestoreBackupResult> {
  return (
    await uploadBackup('/api/admin/backups/restore/execute', token, file, 'RESTORE')
  ).json() as Promise<AdminRestoreBackupResult>
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

export async function syncAdminIndex(target: AdminIndexSyncTarget, token: string): Promise<AdminIndexSyncResult> {
  return (
    await adminRequest('/api/admin/indexes/sync', token, 'POST', { target })
  ).json() as Promise<AdminIndexSyncResult>
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

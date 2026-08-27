import { authHeaders, request } from './api'

export interface AuditEvent {
  id: string
  scope: 'system' | 'room'
  room_id: string | null
  actor_user_id: string
  actor_username: string
  event_type: string
  target_type: string | null
  target_id: string | null
  details: Record<string, string>
  created_at: string
}

export interface AuditEventPage {
  items: AuditEvent[]
  next_cursor: string | null
}

export interface AuditFilters {
  actor?: string
  eventType?: string
  from?: string
  to?: string
  cursor?: string
  limit?: number
}

export function auditParams(filters: AuditFilters = {}): URLSearchParams {
  const params = new URLSearchParams()
  if (filters.actor) params.set('actor', filters.actor)
  if (filters.eventType) params.set('event_type', filters.eventType)
  if (filters.from) params.set('from', filters.from)
  if (filters.to) params.set('to', filters.to)
  if (filters.cursor) params.set('cursor', filters.cursor)
  params.set('limit', String(Math.max(1, Math.min(100, filters.limit ?? 50))))
  return params
}

async function list(path: string, token: string, filters: AuditFilters, signal?: AbortSignal): Promise<AuditEventPage> {
  const response = await request(`${path}?${auditParams(filters)}`, { headers: authHeaders(token), signal })
  if (response.status === 400) throw new Error('审计筛选条件无效')
  if (response.status === 401) throw new Error('登录已过期')
  if (response.status === 403) throw new Error('没有读取审计记录的权限')
  if (response.status === 404) throw new Error('聊天室已不存在')
  if (!response.ok) throw new Error(`读取审计记录失败：${response.status}`)
  return response.json() as Promise<AuditEventPage>
}

export function listSystemAuditEvents(
  token: string,
  filters: AuditFilters = {},
  signal?: AbortSignal,
): Promise<AuditEventPage> {
  return list('/api/admin/audit-events', token, filters, signal)
}

export function listRoomAuditEvents(
  roomId: string,
  token: string,
  filters: AuditFilters = {},
  signal?: AbortSignal,
): Promise<AuditEventPage> {
  return list(`/api/rooms/${encodeURIComponent(roomId)}/audit-events`, token, filters, signal)
}

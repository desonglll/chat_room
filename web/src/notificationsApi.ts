import { authHeaders, request } from './api'

export const NOTIFICATIONS_CHANGED_EVENT = 'chat-room:notifications-changed'

export interface NotificationsChangedSignal {
  type: 'notifications_changed'
  unread_count: number
  latest_notification_id: string | null
}

export type NotificationKind = 'friend_request' | 'room_join_request' | 'mention' | 'reply' | 'ai_run_completed'

export interface NotificationActor {
  id: string
  username: string
  display_name: string
  avatar_emoji: string
}

export interface NotificationItem {
  id: string
  kind: NotificationKind
  actor: NotificationActor | null
  room_id: string | null
  room_name: string | null
  message_id: string | null
  run_id: string | null
  summary: string
  source_available: boolean
  created_at: string
  read_at: string | null
}

export interface NotificationPage {
  items: NotificationItem[]
  next_cursor: string | null
}

export function notificationParams(kind: NotificationKind | '', cursor = '', limit = 30): URLSearchParams {
  const params = new URLSearchParams({ limit: String(limit) })
  if (kind) params.set('kind', kind)
  if (cursor) params.set('cursor', cursor)
  return params
}

async function notificationRequest(path: string, token: string, options: RequestInit = {}): Promise<Response> {
  const response = await request(path, {
    ...options,
    headers: { ...authHeaders(token), ...options.headers },
  })
  if (response.status === 401) throw new Error('登录已过期')
  if (!response.ok) throw new Error(`通知请求失败：${response.status}`)
  return response
}

export async function listNotifications(
  token: string,
  kind: NotificationKind | '' = '',
  cursor = '',
  signal?: AbortSignal,
): Promise<NotificationPage> {
  const response = await notificationRequest(`/api/notifications?${notificationParams(kind, cursor)}`, token, {
    signal,
  })
  return response.json() as Promise<NotificationPage>
}

export async function getNotificationUnreadCount(token: string): Promise<number> {
  const response = await notificationRequest('/api/notifications/unread-count', token)
  const payload = (await response.json()) as { unread_count: number }
  return payload.unread_count
}

export async function markNotificationRead(token: string, id: string): Promise<void> {
  await notificationRequest(`/api/notifications/${encodeURIComponent(id)}/read`, token, { method: 'POST' })
}

export async function markAllNotificationsRead(token: string): Promise<void> {
  await notificationRequest('/api/notifications/read-all', token, { method: 'POST' })
}

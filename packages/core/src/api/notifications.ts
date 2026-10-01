/** TG-703 notification center (`src/notifications`): list, unread count, mark read. */
import { encodePathSegment, QueryParams, type ApiClient } from './http'

export type NotificationKind = 'friend_request' | 'room_join_request' | 'mention' | 'reply' | 'ai_run_completed'

export interface NotificationView {
  id: string
  kind: NotificationKind
  actor: { id: string; username: string; display_name: string; avatar_emoji: string } | null
  room_id: string | null
  room_name: string | null
  message_id: string | null
  run_id: string | null
  summary: string
  /** False once the source message or chat is gone or no longer readable. */
  source_available: boolean
  created_at: string
  read_at: string | null
}

export interface NotificationPage {
  items: NotificationView[]
  next_cursor: string | null
}

export interface NotificationsApi {
  list(cursor?: string): Promise<NotificationPage>
  unreadCount(): Promise<number>
  markRead(id: string): Promise<void>
  markAllRead(): Promise<void>
}

export function createNotificationsApi(client: ApiClient, token: () => string | null): NotificationsApi {
  const auth = () => {
    const value = token()
    return value ? { token: value } : {}
  }
  return {
    list: (cursor) =>
      client.json<NotificationPage>('GET', '/api/notifications', {
        ...auth(),
        query: new QueryParams({ limit: '30', ...(cursor ? { cursor } : {}) }),
      }),
    unreadCount: async () =>
      (await client.json<{ unread_count: number }>('GET', '/api/notifications/unread-count', auth())).unread_count,
    markRead: async (id) => {
      await client.request('POST', `/api/notifications/${encodePathSegment(id)}/read`, auth())
    },
    markAllRead: async () => {
      await client.request('POST', '/api/notifications/read-all', auth())
    },
  }
}

/** Where a notification leads: the message, the chat, the contacts page, or nowhere. */
export function notificationTarget(item: NotificationView): string | null {
  if (item.kind === 'friend_request') return '/contacts'
  if (!item.source_available || !item.room_id) return null
  const chat = `/chat/${encodePathSegment(item.room_id)}`
  return item.message_id ? `${chat}?message=${encodePathSegment(item.message_id)}` : chat
}

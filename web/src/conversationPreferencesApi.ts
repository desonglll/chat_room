import { authHeaders, request } from './api'

export type ConversationNotificationLevel = 'all' | 'mentions' | 'none'

export interface ConversationPreferences {
  room_id: string
  is_pinned: boolean
  is_archived: boolean
  notification_level: ConversationNotificationLevel
  muted_until: string | null
  updated_at: string
}

export interface ConversationPreferencesPatch {
  is_pinned?: boolean
  is_archived?: boolean
  notification_level?: ConversationNotificationLevel
  muted_until?: string | null
}

export async function updateConversationPreferences(
  roomId: string,
  patch: ConversationPreferencesPatch,
  token: string,
): Promise<ConversationPreferences> {
  const response = await request(`/api/conversations/${encodeURIComponent(roomId)}/preferences`, {
    method: 'PATCH',
    headers: { ...authHeaders(token), 'Content-Type': 'application/json' },
    body: JSON.stringify(patch),
  })
  if (response.status === 401) throw new Error('登录已过期')
  if (response.status === 404) throw new Error('会话已失效，请刷新后重试')
  if (!response.ok) throw new Error(`保存会话设置失败：${response.status}`)
  return response.json() as Promise<ConversationPreferences>
}

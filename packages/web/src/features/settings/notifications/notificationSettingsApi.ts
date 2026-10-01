/** TG-508: the viewer's notification settings (defaults per chat type, per-chat exceptions). */
import { authStore, selectToken } from '@tg/core'
import { apiClient } from '../../../app/client'

export type NotificationScope = 'private' | 'group' | 'channel'

export interface NotificationDefaults {
  scope: NotificationScope
  enabled: boolean
  preview: boolean
  sound: string
}

export interface NotificationException {
  enabled?: boolean | null
  preview?: boolean | null
  sound?: string | null
}

export interface NotificationExceptionView extends NotificationException {
  chat_id: string
  chat_title: string
}

export interface NotificationSettings {
  defaults: NotificationDefaults[]
  exceptions: NotificationExceptionView[]
}

export const SOUNDS: { id: string; label: string }[] = [
  { id: 'default', label: '默认' },
  { id: 'note', label: '音符' },
  { id: 'chime', label: '风铃' },
  { id: 'pop', label: '气泡' },
  { id: 'bell', label: '铃声' },
  { id: 'none', label: '无声' },
]

export const SCOPE_LABEL: Record<NotificationScope, string> = { private: '私聊', group: '群组', channel: '频道' }

const auth = () => {
  const token = selectToken(authStore.getState())
  return token ? { token } : {}
}

export const notificationSettingsApi = {
  get: () => apiClient.json<NotificationSettings>('GET', '/api/users/me/notification-settings', auth()),
  setDefaults: (defaults: NotificationDefaults) =>
    apiClient.json<NotificationDefaults>('PUT', `/api/users/me/notification-settings/defaults/${defaults.scope}`, {
      ...auth(),
      body: { enabled: defaults.enabled, preview: defaults.preview, sound: defaults.sound },
    }),
  setException: (chatId: string, exception: NotificationException) =>
    apiClient.json<NotificationException>('PUT', `/api/chats/${encodeURIComponent(chatId)}/notification-exception`, {
      ...auth(),
      body: exception,
    }),
}

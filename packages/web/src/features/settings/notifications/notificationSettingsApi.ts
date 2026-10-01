/** TG-508: the viewer's notification settings (defaults per chat type, per-chat exceptions). */
import { authStore, selectToken } from '@tg/core'
import { apiClient } from '../../../app/client'
import { t } from '../../../i18n/index'

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
  {
    id: 'default',
    get label() {
      return t('w.settings.c8d09c')
    },
  },
  {
    id: 'note',
    get label() {
      return t('w.settings.7ac328')
    },
  },
  {
    id: 'chime',
    get label() {
      return t('w.settings.c07f3d')
    },
  },
  {
    id: 'pop',
    get label() {
      return t('w.settings.d9be7c')
    },
  },
  {
    id: 'bell',
    get label() {
      return t('w.settings.232925')
    },
  },
  {
    id: 'none',
    get label() {
      return t('w.settings.e798b3')
    },
  },
]

export const SCOPE_LABEL: Record<NotificationScope, string> = {
  get private() {
    return t('w.settings.3adfdb')
  },
  get group() {
    return t('w.settings.4260ca')
  },
  get channel() {
    return t('w.settings.b76dfd')
  },
}

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

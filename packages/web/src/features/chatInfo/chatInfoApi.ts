/**
 * The two endpoints the info panel needs that `@tg/core/api` does not wrap yet. Kept
 * beside the feature (AGENTS.md: domain clients beside the domain, not in a shared api
 * file); moving them into core is a one-line re-export for whoever needs them next.
 *
 *   GET   /api/users/:id                        public profile (bio = `signature`)
 *   GET   /api/conversations/:id/preferences    the viewer's own pin/archive/mute row
 *   PATCH /api/conversations/:id/preferences    same, partial update
 *
 * Both are authorised per request server-side (session; preferences only for an active
 * membership — 404 otherwise).
 */
import type { ApiClient, ConversationPreferences, NotificationLevel, User } from '@tg/core'
import { encodePathSegment } from '@tg/core'

export function getUserProfile(client: ApiClient, token: string, userId: string): Promise<User> {
  return client.json<User>('GET', `/api/users/${encodePathSegment(userId)}`, { token })
}

const preferencesPath = (chatId: string) => `/api/conversations/${encodePathSegment(chatId)}/preferences`

export function getConversationPreferences(
  client: ApiClient,
  token: string,
  chatId: string,
): Promise<ConversationPreferences> {
  return client.json<ConversationPreferences>('GET', preferencesPath(chatId), { token })
}

export interface PreferencesPatch {
  notification_level?: NotificationLevel
  /** `null` clears a timed mute. */
  muted_until?: string | null
}

export function updateConversationPreferences(
  client: ApiClient,
  token: string,
  chatId: string,
  patch: PreferencesPatch,
): Promise<ConversationPreferences> {
  return client.json<ConversationPreferences>('PATCH', preferencesPath(chatId), { token, body: patch })
}

/** Telegram's single "通知" switch: on = everything, off = muted indefinitely. */
export function notificationsPatch(enabled: boolean): PreferencesPatch {
  return enabled ? { notification_level: 'all', muted_until: null } : { notification_level: 'none' }
}

export function notificationsEnabled(preferences: ConversationPreferences | null, now: number): boolean {
  if (!preferences) return true
  if (preferences.notification_level === 'none') return false
  return preferences.muted_until === null || Date.parse(preferences.muted_until) <= now
}

/** TG-511: profile photo history and the add-by-QR lookup, on top of the existing APIs. */
import type { User } from '@tg/core'
import { authStore, QueryParams, selectToken } from '@tg/core'
import { apiClient } from '../../app/client'

export interface AvatarHistoryEntry {
  id: string
  url: string
  is_current: boolean
  created_at: string
}

export interface FoundUser {
  id: string
  username: string
  avatar_emoji: string
  display_name: string
  relationship: string
}

const auth = () => {
  const token = selectToken(authStore.getState())
  return token ? { token } : {}
}

export const profileApi = {
  avatars: (userId: string) =>
    apiClient.json<AvatarHistoryEntry[]>('GET', `/api/users/${encodeURIComponent(userId)}/avatars`, auth()),
  setMain: (avatarId: string) =>
    apiClient.json<User>('PUT', `/api/users/me/avatars/${encodeURIComponent(avatarId)}/main`, auth()),
  remove: (avatarId: string) =>
    apiClient.json<User>('DELETE', `/api/users/me/avatars/${encodeURIComponent(avatarId)}`, auth()),
  /** Exact username lookup through the user search (the QR carries the username). */
  findByUsername: async (username: string): Promise<FoundUser | null> => {
    const found = await apiClient.json<FoundUser[]>('GET', '/api/users/search', {
      ...auth(),
      query: new QueryParams({ q: username, limit: '10' }),
    })
    return found.find((user) => user.username.toLowerCase() === username.toLowerCase()) ?? null
  },
  addFriend: (userId: string) =>
    apiClient.json('POST', '/api/friend-requests', { ...auth(), body: { user_id: userId } }),
}

/** What the QR encodes: an absolute link to this app's add-contact page. */
export function addContactLink(origin: string, username: string): string {
  return `${origin.replace(/\/+$/, '')}/add/${encodeURIComponent(username)}`
}

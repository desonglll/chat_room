/** TG-511: profile photo history and the add-by-QR lookup, on top of the existing APIs. */
import type { User } from '@tg/core'
import { ApiError, authStore, QueryParams, selectToken } from '@tg/core'
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
  /**
   * TG-1204: set a new profile photo (`POST /api/users/me/avatar`, multipart `file`). The server
   * keeps the previous ones as history. No React page called this, so nobody could add a photo.
   */
  upload: async (
    file: Blob,
    fetchImpl: (input: string, init: RequestInit) => Promise<Response> = (input, init) => fetch(input, init),
  ): Promise<User> => {
    const path = '/api/users/me/avatar'
    const form = new FormData()
    form.append('file', file, file instanceof File ? file.name : 'avatar')
    const response = await fetchImpl(path, {
      method: 'POST',
      headers: { Authorization: `Bearer ${selectToken(authStore.getState())}`, Accept: 'application/json' },
      body: form,
    })
    if (!response.ok) throw new ApiError(response.status, path, response.statusText)
    return (await response.json()) as User
  },
  addFriend: (userId: string) =>
    apiClient.json('POST', '/api/friend-requests', { ...auth(), body: { user_id: userId } }),
}

/** What the QR encodes: an absolute link to this app's add-contact page. */
export function addContactLink(origin: string, username: string): string {
  return `${origin.replace(/\/+$/, '')}/add/${encodeURIComponent(username)}`
}

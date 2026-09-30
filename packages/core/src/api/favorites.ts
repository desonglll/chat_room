/**
 * TG-503 Saved Messages. `favorites` is the single source of truth (D-010): Saved Messages is
 * a chat-shaped VIEW of it, not a second store. Notes are `manual` favorites; a saved message
 * is a `message` favorite that snapshots its source (sender, chat, content, attachment).
 */
import type { Attachment } from '../types'
import type { ApiClient } from './http'
import { encodePathSegment } from './http'

export type FavoriteKind = 'message' | 'video' | 'manual'

export interface FavoriteItem {
  id: string
  owner_id: string
  owner_username: string
  owner_display_name: string
  /** `owner` or `editor` (a collaborator). */
  access: string
  version: number
  collaborator_count: number
  kind: FavoriteKind
  title: string
  content: string
  source_message_id: string | null
  /** Present only while the viewer can still read the source chat. */
  source_room_id: string | null
  source_sender: string
  source_room_name: string
  attachment: Attachment | null
  created_at: string
  updated_at: string
}

export interface FavoriteForwardResult {
  favorite_id: string
  target_room_id: string
  forwarded_message_id: string | null
  skipped_reason: string | null
}

export interface FavoritesApi {
  list(): Promise<FavoriteItem[]>
  /** A note typed into Saved Messages. */
  addNote(content: string): Promise<FavoriteItem>
  /** Save chat messages (their content and attachments are snapshotted). */
  saveMessages(messageIds: readonly string[]): Promise<FavoriteItem[]>
  remove(favoriteId: string): Promise<void>
  forward(favoriteId: string, targetChatIds: readonly string[]): Promise<FavoriteForwardResult[]>
}

export function createFavoritesApi(client: ApiClient, token: () => string | null): FavoritesApi {
  const auth = () => {
    const value = token()
    return value ? { token: value } : {}
  }
  return {
    list: () => client.json<FavoriteItem[]>('GET', '/api/favorites', auth()),
    addNote: (content) =>
      client.json<FavoriteItem>('POST', '/api/favorites', { ...auth(), body: { title: '', content } }),
    saveMessages: (messageIds) =>
      client.json<FavoriteItem[]>('POST', '/api/favorites/messages', {
        ...auth(),
        body: { message_ids: [...messageIds] },
      }),
    remove: async (favoriteId) => {
      await client.request('DELETE', `/api/favorites/${encodePathSegment(favoriteId)}`, auth())
    },
    forward: (favoriteId, targetChatIds) =>
      client.json<FavoriteForwardResult[]>('POST', `/api/favorites/${encodePathSegment(favoriteId)}/forward`, {
        ...auth(),
        body: { target_room_ids: [...targetChatIds] },
      }),
  }
}

/** Oldest first, as a chat reads (the API answers newest first). */
export function savedMessagesTimeline(items: readonly FavoriteItem[]): FavoriteItem[] {
  return [...items].sort((a, b) => a.created_at.localeCompare(b.created_at) || a.id.localeCompare(b.id))
}

/** The header line of a saved message: «来自 Alice · 项目组»; empty for a note. */
export function savedSourceLine(item: FavoriteItem): string {
  if (item.kind === 'manual') return ''
  if (item.source_sender && item.source_room_name) return `来自 ${item.source_sender} · ${item.source_room_name}`
  if (item.source_sender) return `来自 ${item.source_sender}`
  return item.source_room_name ? `来自 ${item.source_room_name}` : ''
}

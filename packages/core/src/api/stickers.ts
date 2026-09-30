/**
 * Sticker library HTTP client (TG-303) over TG-302's frozen contract
 * (`docs/devlog/TG-302.md` "Frozen interface"). Every library write answers the whole
 * `InstalledStickerSets` with a `revision`, so callers replace their copy instead of
 * patching it; recents and favorites answer 204 and the caller re-reads or applies locally.
 */
import { encodePathSegment, QueryParams, type ApiClient } from './http'
import type { StickerFormat, StoredMessage } from '../types'

export type StickerSetType = 'regular' | 'custom_emoji'

export interface Sticker {
  id: string
  set_id: string
  /** The primary emoji. */
  emoji: string
  /** 1–20 emoji the sticker answers to (suggestions and search). */
  emojis: string[]
  format: StickerFormat
  mime_type: string
  width: number
  height: number
  duration_ms: number | null
  size_bytes: number
  /** Catalogue capability URL (`/api/stickers/:id/file?key=`). */
  file_url: string
}

export interface StickerSet {
  id: string
  short_name: string
  title: string
  set_type: StickerSetType
  owner_id: string | null
  stickers: Sticker[]
  installed: boolean
  archived: boolean
  created_at: string
  updated_at: string
}

/** Installed sets, top first; archived sets are included with `archived: true`. */
export interface InstalledStickerSets {
  revision: number
  sets: StickerSet[]
}

export interface SendStickerInput {
  sticker_id: string
  reply_to?: string | undefined
  client_message_id?: string | undefined
  /** TG-204: the forum topic; omitted for General. */
  topic_id?: string | null | undefined
}

export interface StickersApi {
  installed(): Promise<InstalledStickerSets>
  /** Install (or un-archive) at the top; idempotent. */
  install(setId: string): Promise<InstalledStickerSets>
  uninstall(setId: string): Promise<InstalledStickerSets>
  setArchived(setId: string, archived: boolean): Promise<InstalledStickerSets>
  /** Full new order of installed set ids, top first. */
  reorder(setIds: readonly string[]): Promise<InstalledStickerSets>
  /** A set by its public short name (case-insensitive); `null` when it does not exist. */
  set(shortName: string): Promise<StickerSet | null>
  recent(): Promise<Sticker[]>
  removeRecent(stickerId: string): Promise<void>
  favorites(): Promise<Sticker[]>
  setFavorite(stickerId: string, favorite: boolean): Promise<void>
  /** Stickers from the caller's non-archived installed sets answering to `emoji`. */
  search(emoji: string): Promise<Sticker[]>
  send(chatId: string, input: SendStickerInput): Promise<StoredMessage>
}

export function createStickersApi(client: ApiClient, token: () => string | null): StickersApi {
  const auth = () => {
    const value = token()
    return value ? { token: value } : {}
  }
  const installedPath = (setId: string) => `/api/stickers/installed/${encodePathSegment(setId)}`
  const noContent = async (method: string, path: string) => {
    await client.request(method, path, auth())
  }
  return {
    installed: () => client.json<InstalledStickerSets>('GET', '/api/stickers/installed', auth()),
    install: (setId) => client.json<InstalledStickerSets>('PUT', installedPath(setId), auth()),
    uninstall: (setId) => client.json<InstalledStickerSets>('DELETE', installedPath(setId), auth()),
    setArchived: (setId, archived) =>
      client.json<InstalledStickerSets>('PATCH', installedPath(setId), { ...auth(), body: { archived } }),
    reorder: (setIds) =>
      client.json<InstalledStickerSets>('PUT', '/api/stickers/installed', { ...auth(), body: { set_ids: setIds } }),
    set: async (shortName) => {
      const response = await client.request('GET', `/api/sticker-sets/${encodePathSegment(shortName.trim())}`, {
        ...auth(),
        allowStatuses: [404],
      })
      return response.status === 404 ? null : ((await response.json()) as StickerSet)
    },
    recent: () => client.json<Sticker[]>('GET', '/api/stickers/recent', auth()),
    removeRecent: (stickerId) => noContent('DELETE', `/api/stickers/recent/${encodePathSegment(stickerId)}`),
    favorites: () => client.json<Sticker[]>('GET', '/api/stickers/favorites', auth()),
    setFavorite: (stickerId, favorite) =>
      noContent(favorite ? 'PUT' : 'DELETE', `/api/stickers/favorites/${encodePathSegment(stickerId)}`),
    search: (emoji) =>
      client.json<Sticker[]>('GET', '/api/stickers/search', { ...auth(), query: new QueryParams({ emoji }) }),
    send: (chatId, input) =>
      client.json<StoredMessage>('POST', `/api/chats/${encodePathSegment(chatId)}/sticker-messages`, {
        ...auth(),
        body: {
          sticker_id: input.sticker_id,
          ...(input.reply_to ? { reply_to: input.reply_to } : {}),
          ...(input.client_message_id ? { client_message_id: input.client_message_id } : {}),
          ...(input.topic_id ? { topic_id: input.topic_id } : {}),
        },
      }),
  }
}

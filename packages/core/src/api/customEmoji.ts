/**
 * Custom emoji and emoji status HTTP client (TG-304). Contract frozen in
 * `docs/devlog/TG-304.md`. Custom emoji sets are sticker sets of type `custom_emoji`;
 * installing/archiving them uses the sticker library endpoints (TG-302).
 */
import { QueryParams, type ApiClient } from './http'

export type CustomEmojiFormat = 'webp' | 'tgs' | 'webm'

/** One custom emoji. `file_url` is a capability URL: load it without credentials. */
export interface CustomEmoji {
  id: string
  set_id: string
  set_short_name: string
  /** The fallback Unicode emoji — what copy/paste and unknown-emoji rendering use. */
  emoji: string
  format: CustomEmojiFormat
  width: number
  height: number
  file_url: string
}

/** A sticker of a custom emoji set as `/api/custom-emoji/installed` lists it. */
export interface CustomEmojiSticker {
  id: string
  set_id: string
  emoji: string
  emojis: string[]
  format: CustomEmojiFormat
  width: number
  height: number
  file_url: string
}

export interface CustomEmojiSet {
  id: string
  short_name: string
  title: string
  set_type: 'custom_emoji'
  stickers: CustomEmojiSticker[]
  installed: boolean
  archived: boolean
}

export interface InstalledCustomEmojiSets {
  revision: number
  sets: CustomEmojiSet[]
}

export interface EmojiStatus {
  user_id: string
  custom_emoji_id: string
  /** ISO timestamp, or null for "until cleared". */
  expires_at: string | null
  emoji: CustomEmoji
}

/** Server ceiling for one `ids` lookup. */
export const MAX_CUSTOM_EMOJI_LOOKUP = 200

const idQuery = (ids: readonly string[]) => new QueryParams({ ids: ids.join(',') })

/** Resolve ids to files; unknown or removed ids are absent from the answer. */
export function resolveCustomEmoji(client: ApiClient, token: string, ids: readonly string[]): Promise<CustomEmoji[]> {
  if (ids.length === 0) return Promise.resolve([])
  return client.json<CustomEmoji[]>('GET', '/api/custom-emoji', { token, query: idQuery(ids) })
}

export function listInstalledCustomEmoji(client: ApiClient, token: string): Promise<InstalledCustomEmojiSets> {
  return client.json<InstalledCustomEmojiSets>('GET', '/api/custom-emoji/installed', { token })
}

/** Statuses the caller may see among `userIds`; others are simply absent. */
export function getEmojiStatuses(client: ApiClient, token: string, userIds: readonly string[]): Promise<EmojiStatus[]> {
  if (userIds.length === 0) return Promise.resolve([])
  return client.json<EmojiStatus[]>('GET', '/api/users/emoji-statuses', { token, query: idQuery(userIds) })
}

export function setEmojiStatus(
  client: ApiClient,
  token: string,
  status: { custom_emoji_id: string; expires_at?: string | null },
): Promise<EmojiStatus> {
  return client.json<EmojiStatus>('PUT', '/api/users/me/emoji-status', { token, body: status })
}

export async function clearEmojiStatus(client: ApiClient, token: string): Promise<void> {
  await client.request('DELETE', '/api/users/me/emoji-status', { token })
}

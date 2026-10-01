/**
 * TG-504: the search tabs and the recent-search list. Pure; the tabs map onto the server's
 * `content_type` filter so each tab pages on its own cursor.
 */
import type { CoreStorage, GlobalSearchContentType } from '@tg/core'

export type SearchTabId = 'chats' | 'messages' | 'media' | 'links' | 'files' | 'music' | 'voice'

export interface SearchTab {
  id: SearchTabId
  label: string
  /** The server filter; `null` for the client-side chat list. */
  contentType: GlobalSearchContentType | null
}

export const SEARCH_TABS: readonly SearchTab[] = [
  { id: 'chats', label: '聊天', contentType: null },
  { id: 'messages', label: '消息', contentType: 'all' },
  { id: 'media', label: '媒体', contentType: 'media' },
  { id: 'links', label: '链接', contentType: 'link' },
  { id: 'files', label: '文件', contentType: 'document' },
  { id: 'music', label: '音乐', contentType: 'music' },
  { id: 'voice', label: '语音', contentType: 'voice' },
]

export const RECENT_SEARCH_KEY = 'tg.search.recent.v1'
export const RECENT_SEARCH_MAX = 10

export function readRecentSearches(storage: CoreStorage): string[] {
  try {
    const parsed = JSON.parse(storage.getItem(RECENT_SEARCH_KEY) ?? '[]') as unknown
    return Array.isArray(parsed) ? parsed.filter((item): item is string => typeof item === 'string') : []
  } catch {
    return []
  }
}

/** Newest first, without duplicates (case-insensitive), at most {@link RECENT_SEARCH_MAX}. */
export function rememberSearch(recent: readonly string[], query: string): string[] {
  const trimmed = query.trim()
  if (!trimmed) return [...recent]
  const key = trimmed.toLocaleLowerCase()
  return [trimmed, ...recent.filter((item) => item.toLocaleLowerCase() !== key)].slice(0, RECENT_SEARCH_MAX)
}

/** Suggestions while typing: recent searches that extend what is typed (not equal to it). */
export function suggestSearches(recent: readonly string[], query: string, limit = 3): string[] {
  const key = query.trim().toLocaleLowerCase()
  if (!key) return recent.slice(0, limit)
  return recent
    .filter((item) => item.toLocaleLowerCase().startsWith(key) && item.toLocaleLowerCase() !== key)
    .slice(0, limit)
}

/** A `yyyy-mm-dd` day from an ISO timestamp, local time — for the result's date line. */
export function resultDay(iso: string): string {
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return ''
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`
}

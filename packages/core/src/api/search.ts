/**
 * Global message search (`/api/messages/search`). The params/day-boundary logic and the
 * result types migrated verbatim from `web/src/globalSearchApi.ts` (TG-011, with its
 * test); the transport was rewritten on the shared `ApiClient`.
 */
import { QueryParams, type ApiClient } from './http'

/** TG-504 adds the tab kinds: media (photo+video), document, link, music, voice. */
export type GlobalSearchContentType =
  | 'all'
  | 'text'
  | 'file'
  | 'image'
  | 'video'
  | 'audio'
  | 'media'
  | 'document'
  | 'link'
  | 'music'
  | 'voice'

export interface GlobalSearchFilters {
  q: string
  roomId: string
  senderId: string
  from: string
  to: string
  contentType: GlobalSearchContentType
}

export interface GlobalSearchResult {
  message_id: string
  room_id: string
  conversation_kind: 'group' | 'direct'
  conversation_title: string
  sender_id: string | null
  sender: string
  excerpt: string
  content_type: GlobalSearchContentType
  attachment_file_name: string | null
  context_before: string | null
  context_after: string | null
  created_at: string
}

export interface GlobalSearchPage {
  items: GlobalSearchResult[]
  next_cursor: string | null
}

function dayBoundary(value: string, endOfDay: boolean): string {
  const [year, month, day] = value.split('-').map(Number)
  const date = new Date(
    year as number,
    (month as number) - 1,
    day as number,
    endOfDay ? 23 : 0,
    endOfDay ? 59 : 0,
    endOfDay ? 59 : 0,
    endOfDay ? 999 : 0,
  )
  return date.toISOString()
}

export function globalSearchParams(filters: GlobalSearchFilters, cursor = '', limit = 30): QueryParams {
  const params = new QueryParams({ q: filters.q.trim(), limit: String(limit) })
  if (filters.roomId) params.set('room_id', filters.roomId)
  if (filters.senderId) params.set('sender_id', filters.senderId)
  if (filters.from) params.set('from', dayBoundary(filters.from, false))
  if (filters.to) params.set('to', dayBoundary(filters.to, true))
  if (filters.contentType !== 'all') params.set('content_type', filters.contentType)
  if (cursor) params.set('cursor', cursor)
  return params
}

export function searchGlobalMessages(
  client: ApiClient,
  token: string,
  filters: GlobalSearchFilters,
  cursor = '',
  limit = 30,
): Promise<GlobalSearchPage> {
  return client.json<GlobalSearchPage>('GET', '/api/messages/search', {
    token,
    query: globalSearchParams(filters, cursor, limit),
  })
}

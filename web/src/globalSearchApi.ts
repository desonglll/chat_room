import { authHeaders, request } from './api'

export type GlobalSearchContentType = 'all' | 'text' | 'file' | 'image' | 'video' | 'audio'

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
    year,
    month - 1,
    day,
    endOfDay ? 23 : 0,
    endOfDay ? 59 : 0,
    endOfDay ? 59 : 0,
    endOfDay ? 999 : 0,
  )
  return date.toISOString()
}

export function globalSearchParams(filters: GlobalSearchFilters, cursor = '', limit = 30): URLSearchParams {
  const params = new URLSearchParams({ q: filters.q.trim(), limit: String(limit) })
  if (filters.roomId) params.set('room_id', filters.roomId)
  if (filters.senderId) params.set('sender_id', filters.senderId)
  if (filters.from) params.set('from', dayBoundary(filters.from, false))
  if (filters.to) params.set('to', dayBoundary(filters.to, true))
  if (filters.contentType !== 'all') params.set('content_type', filters.contentType)
  if (cursor) params.set('cursor', cursor)
  return params
}

export async function searchGlobalMessages(
  token: string,
  filters: GlobalSearchFilters,
  cursor = '',
  signal?: AbortSignal,
): Promise<GlobalSearchPage> {
  const response = await request(`/api/messages/search?${globalSearchParams(filters, cursor)}`, {
    headers: authHeaders(token),
    signal,
  })
  if (response.status === 400) throw new Error('搜索条件无效，请检查日期和筛选项')
  if (response.status === 401) throw new Error('登录已过期')
  if (response.status === 504) throw new Error('搜索超时，请缩小范围后重试')
  if (!response.ok) throw new Error(`搜索消息失败：${response.status}`)
  return response.json() as Promise<GlobalSearchPage>
}

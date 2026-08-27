import { afterEach, describe, expect, mock, test } from 'bun:test'
import { globalSearchParams, searchGlobalMessages, type GlobalSearchFilters } from './globalSearchApi'

const originalFetch = globalThis.fetch
const filters: GlobalSearchFilters = {
  q: '  exact%_term  ',
  roomId: 'room-1',
  senderId: 'user-1',
  from: '2026-08-01',
  to: '2026-08-02',
  contentType: 'image',
}

afterEach(() => {
  globalThis.fetch = originalFetch
})

describe('global search API', () => {
  test('serializes filters, local-day bounds, and the opaque cursor', () => {
    const params = globalSearchParams(filters, '2026-08-02T12:00:00Z|message-1')
    expect(params.get('q')).toBe('exact%_term')
    expect(params.get('room_id')).toBe('room-1')
    expect(params.get('sender_id')).toBe('user-1')
    expect(new Date(params.get('from')!).getHours()).toBe(0)
    expect(new Date(params.get('to')!).getHours()).toBe(23)
    expect(params.get('content_type')).toBe('image')
    expect(params.get('cursor')).toBe('2026-08-02T12:00:00Z|message-1')
  })

  test('uses the domain endpoint without exposing response content elsewhere', async () => {
    const fetchMock = mock(async () => Response.json({ items: [], next_cursor: null }))
    globalThis.fetch = fetchMock as typeof fetch

    await searchGlobalMessages('session-token', filters)

    const [path, options] = fetchMock.mock.calls[0]!
    expect(String(path)).toStartWith('/api/messages/search?')
    expect(String(path)).toContain('q=exact%25_term')
    expect(options?.headers).toEqual({ Accept: 'application/json', Authorization: 'Bearer session-token' })
  })

  test('reports server timeouts with an actionable error', async () => {
    globalThis.fetch = mock(async () => new Response(null, { status: 504 })) as typeof fetch
    expect(searchGlobalMessages('session-token', filters)).rejects.toThrow('搜索超时')
  })
})

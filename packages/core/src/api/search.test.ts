// Migrated from web/src/globalSearchApi.test.ts (TG-011): the params/day-boundary
// assertions are verbatim; the transport assertions now go through the injected-fetch
// ApiClient instead of patching globalThis.fetch, and error copy is ApiError (see devlog).
import { describe, expect, test } from 'bun:test'
import { createApiClient } from './http'
import { globalSearchParams, searchGlobalMessages, type GlobalSearchFilters } from './search'

const filters: GlobalSearchFilters = {
  q: '  exact%_term  ',
  roomId: 'room-1',
  senderId: 'user-1',
  from: '2026-08-01',
  to: '2026-08-02',
  contentType: 'image',
}

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

  test('uses the domain endpoint with the session token', async () => {
    const urls: string[] = []
    let headers: Record<string, string> = {}
    const client = createApiClient({
      fetchImpl: async (url, init) => {
        urls.push(url)
        headers = init?.headers as Record<string, string>
        return Response.json({ items: [], next_cursor: null })
      },
    })

    await searchGlobalMessages(client, 'session-token', filters)

    expect(urls[0]).toStartWith('/api/messages/search?')
    expect(urls[0]).toContain('q=exact%25_term')
    expect(headers.Authorization).toBe('Bearer session-token')
  })

  test('reports server failures as ApiError with the status', async () => {
    const client = createApiClient({ fetchImpl: async () => new Response(null, { status: 504 }) })
    await expect(searchGlobalMessages(client, 'session-token', filters)).rejects.toMatchObject({
      name: 'ApiError',
      status: 504,
    })
  })
})

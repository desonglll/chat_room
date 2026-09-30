import { describe, expect, test } from 'bun:test'
import { ApiError, createApiClient } from './http'
import { createStickersApi } from './stickers'

function fakeClient(status = 200, body: unknown = { revision: 1, sets: [] }) {
  const calls: Array<{ url: string; method: string; body: unknown; auth: string | undefined }> = []
  const client = createApiClient({
    fetchImpl: async (url, init) => {
      const headers = init?.headers as Record<string, string>
      calls.push({
        url,
        method: String(init?.method),
        body: init?.body === undefined ? undefined : JSON.parse(String(init.body)),
        auth: headers.Authorization,
      })
      return status === 204 ? new Response(null, { status }) : Response.json(body, { status })
    },
  })
  return { calls, api: createStickersApi(client, () => 'tok') }
}

describe('stickers api', () => {
  test('library writes hit the frozen TG-302 paths', async () => {
    const { calls, api } = fakeClient()
    await api.installed()
    await api.install('s 1')
    await api.setArchived('s1', true)
    await api.uninstall('s1')
    await api.reorder(['b', 'a'])
    expect(calls.map((call) => `${call.method} ${call.url}`)).toEqual([
      'GET /api/stickers/installed',
      'PUT /api/stickers/installed/s%201',
      'PATCH /api/stickers/installed/s1',
      'DELETE /api/stickers/installed/s1',
      'PUT /api/stickers/installed',
    ])
    expect(calls[2]?.body).toEqual({ archived: true })
    expect(calls[4]?.body).toEqual({ set_ids: ['b', 'a'] })
    expect(calls.every((call) => call.auth === 'Bearer tok')).toBe(true)
  })

  test('recents, favorites and search', async () => {
    const { calls, api } = fakeClient(204)
    await api.removeRecent('x')
    await api.setFavorite('x', true)
    await api.setFavorite('x', false)
    expect(calls.map((call) => `${call.method} ${call.url}`)).toEqual([
      'DELETE /api/stickers/recent/x',
      'PUT /api/stickers/favorites/x',
      'DELETE /api/stickers/favorites/x',
    ])
    const search = fakeClient(200, [])
    await search.api.search('😀')
    expect(search.calls[0]?.url).toBe(`/api/stickers/search?emoji=${encodeURIComponent('😀')}`)
  })

  test('a missing set is null; other failures throw', async () => {
    const missing = fakeClient(404, { error: 'sticker_set_not_found' })
    expect(await missing.api.set(' Cats ')).toBeNull()
    expect(missing.calls[0]?.url).toBe('/api/sticker-sets/Cats')
    const broken = fakeClient(500, { error: 'boom' })
    await expect(broken.api.set('cats')).rejects.toBeInstanceOf(ApiError)
  })

  test('send posts only the fields that are set', async () => {
    const { calls, api } = fakeClient(201, { id: 'm1' })
    await api.send('c1', { sticker_id: 'st', reply_to: undefined, client_message_id: 'cm' })
    expect(calls[0]?.url).toBe('/api/chats/c1/sticker-messages')
    expect(calls[0]?.body).toEqual({ sticker_id: 'st', client_message_id: 'cm' })
  })
})

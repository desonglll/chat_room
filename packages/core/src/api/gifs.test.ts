import { describe, expect, test } from 'bun:test'
import { ApiError, createApiClient, type FetchLike } from './http'
import { createGifsApi } from './gifs'

interface Call {
  url: string
  method: string
  body: unknown
  headers: Record<string, string>
}

function fake(status = 200, body: unknown = []) {
  const calls: Call[] = []
  const fetchImpl: FetchLike = async (url, init) => {
    calls.push({
      url,
      method: String(init?.method),
      body: init?.body,
      headers: init?.headers as Record<string, string>,
    })
    return status === 204 ? new Response(null, { status }) : Response.json(body, { status })
  }
  const client = createApiClient({ fetchImpl })
  return { calls, api: createGifsApi(client, () => 'tok', fetchImpl) }
}

describe('gifs api', () => {
  test('saved GIFs: list, save, remove', async () => {
    const { calls, api } = fake()
    await api.saved()
    await api.save('m1')
    await api.remove('g 1')
    expect(calls.map((call) => `${call.method} ${call.url}`)).toEqual([
      'GET /api/gifs/saved',
      'POST /api/gifs/saved',
      'DELETE /api/gifs/saved/g%201',
    ])
    expect(JSON.parse(String(calls[1]?.body))).toEqual({ message_id: 'm1' })
    expect(calls.every((call) => call.headers.Authorization === 'Bearer tok')).toBe(true)
  })

  test('recent and send by reference', async () => {
    const { calls, api } = fake(201, { id: 'x' })
    await api.recent(20)
    await api.send('c1', { saved_gif_id: 'g1' }, { reply_to: 'r1', client_message_id: 'u1' })
    await api.send('c1', { message_id: 'm1' })
    expect(calls[0]?.url).toBe('/api/gifs/recent?limit=20')
    expect(calls[1]?.url).toBe('/api/chats/c1/gif-messages')
    expect(JSON.parse(String(calls[1]?.body))).toEqual({ saved_gif_id: 'g1', reply_to: 'r1', client_message_id: 'u1' })
    expect(JSON.parse(String(calls[2]?.body))).toEqual({ message_id: 'm1' })
  })

  test('upload sends the raw body with the options in the query', async () => {
    const { calls, api } = fake(201, { id: 'x' })
    await api.upload('c1', 'BYTES', { client_message_id: 'u1' })
    expect(calls[0]?.url).toBe('/api/chats/c1/gif-messages/upload?client_message_id=u1')
    expect(calls[0]?.body).toBe('BYTES')
    expect(calls[0]?.headers.Authorization).toBe('Bearer tok')
  })

  test('an upload refusal surfaces the server code', async () => {
    const { api } = fake(400, { error: 'audio_not_allowed' })
    const error = await api.upload('c1', 'BYTES').catch((caught: unknown) => caught)
    expect(error).toBeInstanceOf(ApiError)
    expect((error as ApiError).serverMessage).toBe('audio_not_allowed')
  })
})

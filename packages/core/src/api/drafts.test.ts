import { describe, expect, test } from 'bun:test'
import { createDraftsApi, DraftsApiError, type ChatDraft, type DraftFetchLike } from './drafts'

interface RecordedCall {
  url: string
  init: Parameters<DraftFetchLike>[1]
}

const draft: ChatDraft = {
  user_id: 'u1',
  text: 'unsent',
  reply_to_message_id: null,
  topic_id: null,
  updated_at: '2026-09-30T12:00:00Z',
}

const fetchReturning = (status: number, body: unknown) => {
  const calls: RecordedCall[] = []
  const fetchLike: DraftFetchLike = (url, init) => {
    calls.push({ url, init })
    return Promise.resolve({ status, json: () => Promise.resolve(body) })
  }
  return { calls, fetchLike }
}

describe('createDraftsApi', () => {
  test('GET hits the canonical path and parses the stored draft', async () => {
    const { calls, fetchLike } = fetchReturning(200, draft)
    const api = createDraftsApi(fetchLike, { baseUrl: 'https://host' })
    expect(await api.get('c1')).toEqual(draft)
    expect(calls).toHaveLength(1)
    expect(calls[0]?.url).toBe('https://host/api/chats/c1/draft')
    expect(calls[0]?.init.method).toBe('GET')
    expect(calls[0]?.init.body).toBeUndefined()
  })

  test('GET passes the server\'s "no draft stored" null through unchanged', async () => {
    const { fetchLike } = fetchReturning(200, null)
    expect(await createDraftsApi(fetchLike).get('c1')).toBeNull()
  })

  test('PUT sends the write as JSON and returns the stored draft', async () => {
    const { calls, fetchLike } = fetchReturning(200, draft)
    const api = createDraftsApi(fetchLike)
    const body = { text: 'unsent', reply_to_message_id: null }
    expect(await api.put('c1', body)).toEqual(draft)
    expect(calls[0]?.url).toBe('/api/chats/c1/draft')
    expect(calls[0]?.init.method).toBe('PUT')
    expect(JSON.parse(calls[0]?.init.body ?? '')).toEqual(body)
    expect(calls[0]?.init.headers['content-type']).toBe('application/json')
  })

  test('the bearer token is read per request, so a refreshed session needs no new client', async () => {
    const { calls, fetchLike } = fetchReturning(200, null)
    let token: string | null = null
    const api = createDraftsApi(fetchLike, { token: () => token })
    await api.get('c1')
    token = 't-2'
    await api.get('c1')
    expect(calls[0]?.init.headers['authorization']).toBeUndefined()
    expect(calls[1]?.init.headers['authorization']).toBe('Bearer t-2')
  })

  test('a non-200 answer raises a typed error carrying the status and operation', async () => {
    const { fetchLike } = fetchReturning(403, null)
    const api = createDraftsApi(fetchLike)
    const failure = await api.put('c1', { text: 'x' }).catch((error: unknown) => error)
    expect(failure).toBeInstanceOf(DraftsApiError)
    expect((failure as DraftsApiError).status).toBe(403)
    expect((failure as DraftsApiError).operation).toBe('put')
  })
})

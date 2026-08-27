import { afterEach, describe, expect, test } from 'bun:test'
import { createCatchUpRun } from './aiThreadApi'

const originalFetch = globalThis.fetch
afterEach(() => (globalThis.fetch = originalFetch))

describe('trusted catch-up API', () => {
  test('sends only the room, model and idempotency key', async () => {
    let request: RequestInit | undefined
    globalThis.fetch = (async (_path: string | URL | Request, init?: RequestInit) => {
      request = init
      return Response.json({ id: 'run-1' }, { status: 202 })
    }) as typeof fetch
    await createCatchUpRun('token', 'thread/1', 'room-1', '', 'request-1', 'model-1')
    expect(JSON.parse(String(request?.body))).toEqual({
      room_id: 'room-1',
      client_request_id: 'request-1',
      model_option_id: 'model-1',
    })
  })

  test('returns null for a zero-unread response', async () => {
    globalThis.fetch = (async () => new Response(null, { status: 204 })) as typeof fetch
    expect(await createCatchUpRun('token', 'thread-1', 'room-1', '', 'request-1', null)).toBeNull()
  })
})

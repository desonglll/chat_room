// TG-404: the scheduled-messages client against an injected fake fetch — paths, methods, bodies.
import { describe, expect, test } from 'bun:test'
import { createApiClient } from './http'
import {
  createScheduledMessage,
  deleteScheduledMessage,
  listScheduledMessages,
  sendScheduledMessageNow,
  updateScheduledMessage,
} from './scheduled'

function fakeFetch(status = 200, body: unknown = {}) {
  const calls: Array<{ url: string; init: RequestInit | undefined }> = []
  const client = createApiClient({
    fetchImpl: async (url, init) => {
      calls.push({ url, init })
      return status === 204 ? new Response(null, { status }) : Response.json(body, { status })
    },
  })
  return { calls, client }
}

const bodyOf = (init: RequestInit | undefined) => JSON.parse(String(init?.body)) as unknown

describe('scheduled messages api', () => {
  test('creates with the wire field names under the chat', async () => {
    const { calls, client } = fakeFetch(201, { id: 's1' })
    await createScheduledMessage(client, 't', 'chat/1', {
      content: 'later',
      scheduled_at: '2030-01-01T09:00:00.000Z',
      silent: true,
    })
    expect(calls[0]!.url).toBe('/api/chats/chat%2F1/scheduled-messages')
    expect(calls[0]!.init?.method).toBe('POST')
    expect(bodyOf(calls[0]!.init)).toEqual({
      content: 'later',
      scheduled_at: '2030-01-01T09:00:00.000Z',
      silent: true,
    })
  })

  test('list, update, delete and send-now address one scheduled message', async () => {
    const { calls, client } = fakeFetch()
    await listScheduledMessages(client, 't', 'c')
    await updateScheduledMessage(client, 't', 'c', 's1', { scheduled_at: '2030-01-02T00:00:00.000Z' })
    await sendScheduledMessageNow(client, 't', 'c', 's1')
    expect(calls.map((call) => `${call.init?.method} ${call.url}`)).toEqual([
      'GET /api/chats/c/scheduled-messages',
      'PATCH /api/chats/c/scheduled-messages/s1',
      'POST /api/chats/c/scheduled-messages/s1/send-now',
    ])
    expect(bodyOf(calls[1]!.init)).toEqual({ scheduled_at: '2030-01-02T00:00:00.000Z' })
  })

  test('delete tolerates an empty 204 body', async () => {
    const { calls, client } = fakeFetch(204)
    await deleteScheduledMessage(client, 't', 'c', 's1')
    expect(calls[0]!.init?.method).toBe('DELETE')
    expect(calls[0]!.url).toBe('/api/chats/c/scheduled-messages/s1')
  })
})

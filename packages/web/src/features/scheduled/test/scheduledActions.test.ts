// TG-404: scheduled actions keep the store in step with the server, including «already gone».
import { describe, expect, test } from 'bun:test'
import type { ScheduledMessage } from '@tg/core'
import { createApiClient } from '@tg/core'
import { createScheduledActions } from '../scheduledActions'
import { createScheduledStore } from '../scheduledStore'

const item = (id: string, at: string, chat = 'c1'): ScheduledMessage => ({
  id,
  chat_id: chat,
  content: `text ${id}`,
  reply_to: null,
  silent: false,
  scheduled_at: at,
  created_at: at,
  updated_at: at,
})

function setup(respond: (method: string, url: string, body: unknown) => { status: number; body?: unknown }) {
  const calls: string[] = []
  const client = createApiClient({
    fetchImpl: async (url, init) => {
      const method = init?.method ?? 'GET'
      calls.push(`${method} ${url}`)
      const body = init?.body ? (JSON.parse(String(init.body)) as unknown) : undefined
      const reply = respond(method, url, body)
      return reply.status === 204
        ? new Response(null, { status: 204 })
        : Response.json(reply.body ?? {}, { status: reply.status })
    },
  })
  const store = createScheduledStore()
  const actions = createScheduledActions({ client, token: () => 't', store })
  return { calls, store, actions }
}

describe('scheduled actions', () => {
  test('load sorts by time; schedule sends the ISO time and upserts', async () => {
    const { store, actions, calls } = setup((method, _url, body) => {
      if (method === 'GET')
        return { status: 200, body: [item('b', '2030-01-02T00:00:00Z'), item('a', '2030-01-01T00:00:00Z')] }
      const input = body as { content: string; scheduled_at: string; reply_to?: string; silent?: boolean }
      expect(input).toEqual({ content: 'hi', scheduled_at: '2030-01-01T12:00:00.000Z', reply_to: 'm1', silent: true })
      return { status: 201, body: item('n', input.scheduled_at) }
    })
    await actions.load('c1')
    expect(store.getState().byChat.c1?.map((entry) => entry.id)).toEqual(['a', 'b'])
    await actions.schedule('c1', { content: 'hi', replyTo: 'm1', at: new Date('2030-01-01T12:00:00Z'), silent: true })
    expect(store.getState().byChat.c1?.map((entry) => entry.id)).toEqual(['a', 'n', 'b'])
    expect(calls).toEqual(['GET /api/chats/c1/scheduled-messages', 'POST /api/chats/c1/scheduled-messages'])
  })

  test('send-now and delete drop the entry, also when the server says it is already gone', async () => {
    let status = 200
    const { store, actions } = setup(() => ({ status, body: { id: 'a' } }))
    store.getState().replace('c1', [item('a', '2030-01-01T00:00:00Z'), item('b', '2030-01-02T00:00:00Z')])
    await actions.sendNow('c1', 'a')
    expect(store.getState().byChat.c1?.map((entry) => entry.id)).toEqual(['b'])
    status = 404
    await actions.remove('c1', 'b')
    expect(store.getState().byChat.c1).toEqual([])
  })

  test('a failed update on a delivered entry removes it and still reports the error', async () => {
    const { store, actions } = setup(() => ({ status: 404 }))
    store.getState().replace('c1', [item('a', '2030-01-01T00:00:00Z')])
    await expect(actions.update('c1', 'a', { content: 'x' })).rejects.toThrow()
    expect(store.getState().byChat.c1).toEqual([])
  })

  test('delivered broadcasts sweep the list', () => {
    const store = createScheduledStore()
    store.getState().replace('c1', [item('a', '2030-01-01T00:00:00Z'), item('b', '2030-01-02T00:00:00Z')])
    store.getState().dropDelivered('c1', new Set(['a', 'zzz']))
    expect(store.getState().byChat.c1?.map((entry) => entry.id)).toEqual(['b'])
  })
})

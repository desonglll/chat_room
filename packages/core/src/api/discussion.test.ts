import { describe, expect, test } from 'bun:test'
import { createDiscussionApi } from './discussion'
import type { ApiClient } from './http'

function recordingClient() {
  const calls: Array<{ method: string; path: string; body?: unknown }> = []
  const client = {
    json: async (method: string, path: string, options: { body?: unknown } = {}) => {
      calls.push({ method, path, body: options.body })
      return {}
    },
    request: async (method: string, path: string, options: { body?: unknown } = {}) => {
      calls.push({ method, path, body: options.body })
      return new Response(null, { status: 200 })
    },
  } as unknown as ApiClient
  return { client, calls }
}

describe('TG-203 discussion client', () => {
  test('speaks the frozen routes', async () => {
    const { client, calls } = recordingClient()
    const api = createDiscussionApi(client, () => 't')
    await api.link('c1', 'g1')
    await api.link('c1', null)
    await api.thread('c1', 'p1')
    await api.comment('c1', 'p1', { content: 'hi', reply_to: 'm1' })
    await api.join('g1')
    expect(calls).toEqual([
      { method: 'PUT', path: '/api/chats/c1/discussion', body: { chat_id: 'g1' } },
      { method: 'PUT', path: '/api/chats/c1/discussion', body: { chat_id: null } },
      { method: 'GET', path: '/api/chats/c1/posts/p1/comments', body: undefined },
      { method: 'POST', path: '/api/chats/c1/posts/p1/comments', body: { content: 'hi', reply_to: 'm1' } },
      { method: 'POST', path: '/api/chats/g1/join-requests', body: {} },
    ])
  })
})

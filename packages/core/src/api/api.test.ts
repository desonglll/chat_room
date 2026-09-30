// TG-011: the api layer against an injected fake fetch — canonical paths, frozen headers,
// canonical request fields, and ApiError semantics. No global is patched: the fetch
// implementation is a constructor argument, which is the whole point of the rewrite.
import { describe, expect, test } from 'bun:test'
import { loginUser } from './auth'
import { createChat, getChat, listChats, updateChatMember } from './chats'
import { ApiError, createApiClient } from './http'
import { forwardMessages, listChatMessageContext, listChatMessages } from './messages'

interface RecordedCall {
  url: string
  init: RequestInit | undefined
}

function fakeFetch(respond: (url: string) => Response) {
  const calls: RecordedCall[] = []
  const client = createApiClient({
    fetchImpl: async (url, init) => {
      calls.push({ url, init })
      return respond(url)
    },
  })
  return { calls, client }
}

const ok = (body: unknown) => () => Response.json(body)

describe('api client', () => {
  test('targets the canonical /api/chats dialect and sends title, never name', async () => {
    const { calls, client } = fakeFetch(ok({ id: 'chat-1' }))
    await createChat(client, 'token-1', { title: '发布计划', password: null, join_policy: 'open' })

    const call = calls[0]!
    expect(call.url).toBe('/api/chats')
    expect(call.init?.method).toBe('POST')
    const body = JSON.parse(String(call.init?.body)) as Record<string, unknown>
    expect(body.title).toBe('发布计划')
    expect('name' in body).toBe(false)
    expect((call.init?.headers as Record<string, string>).Authorization).toBe('Bearer token-1')
  })

  test('encodes path segments and keeps the frozen x-room-password header spelling', async () => {
    const { calls, client } = fakeFetch(ok([]))
    await listChatMessages(client, 'chat/../1', { token: 't', password: 'pw' }, '2026-09-30T12:00:00Z', 10)

    const call = calls[0]!
    expect(call.url).toBe('/api/chats/chat%2F..%2F1/messages?limit=10&before=2026-09-30T12%3A00%3A00Z')
    expect((call.init?.headers as Record<string, string>)['x-room-password']).toBe('pw')
  })

  test('forward requests keep the frozen target_room_ids field', async () => {
    const { calls, client } = fakeFetch(ok([]))
    await forwardMessages(client, 't', ['m1'], ['c1'])
    expect(JSON.parse(String(calls[0]!.init?.body))).toEqual({ message_ids: ['m1'], target_room_ids: ['c1'] })
  })

  test('login posts to /api/users/login without an auth header', async () => {
    const { calls, client } = fakeFetch(ok({ token: 't', user: {}, expires_at: '' }))
    await loginUser(client, 'alice', 'secret-password')
    const headers = calls[0]!.init?.headers as Record<string, string>
    expect(calls[0]!.url).toBe('/api/users/login')
    expect(headers.Authorization).toBeUndefined()
  })

  test('surfaces failures as ApiError with status and the server error field', async () => {
    const { client } = fakeFetch(() => Response.json({ error: 'chat title already exists' }, { status: 409 }))
    const attempt = createChat(client, 't', { title: 'dup', password: null, join_policy: 'open' })
    await expect(attempt).rejects.toMatchObject({
      name: 'ApiError',
      status: 409,
      path: '/api/chats',
      serverMessage: 'chat title already exists',
    })
    await expect(attempt).rejects.toBeInstanceOf(ApiError)
  })

  test('allows caller-handled statuses: a missing chat is null, missing context is empty', async () => {
    const { client } = fakeFetch(() => new Response(null, { status: 404 }))
    expect(await getChat(client, 'gone', 't')).toBeNull()
    expect(await listChatMessageContext(client, 'c', 'gone', { token: 't' })).toEqual([])
  })

  test('member moderation PATCHes the canonical members path', async () => {
    const { calls, client } = fakeFetch(ok({}))
    await updateChatMember(client, 'chat-1', 'user-2', 't', 'set_role', 'admin')
    expect(calls[0]!.url).toBe('/api/chats/chat-1/members/user-2')
    expect(JSON.parse(String(calls[0]!.init?.body))).toEqual({ action: 'set_role', role: 'admin' })
  })

  test('unauthenticated listChats sends no Authorization header', async () => {
    const { calls, client } = fakeFetch(ok([]))
    await listChats(client)
    expect((calls[0]!.init?.headers as Record<string, string>).Authorization).toBeUndefined()
  })
})

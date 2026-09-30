import { describe, expect, test } from 'bun:test'
import { ApiError, createApiClient } from './http'
import { createInviteLinksApi, inviteLinkPath, inviteLinkUrl, inviteRefusal } from './inviteLinks'

function fakeClient(body: unknown, status = 200) {
  const calls: Array<{ url: string; init: RequestInit | undefined }> = []
  const client = createApiClient({
    fetchImpl: async (url, init) => {
      calls.push({ url, init })
      return status === 204 ? new Response(null, { status }) : Response.json(body, { status })
    },
  })
  return { calls, client }
}

const input = { title: '朋友', expires_at: null, usage_limit: 5, requires_approval: false }

describe('invite links api', () => {
  test('management hits the frozen paths with the bearer token and bodies', async () => {
    const { calls, client } = fakeClient({ links: [] })
    const api = createInviteLinksApi(client, () => 'tok')
    await api.list('c1')
    await api.create('c1', input)
    await api.edit('c1', 'l1', input)
    await api.revoke('c1', 'l1')
    await api.replacePrimary('c1')
    await api.members('c1', 'l1')
    await api.requests('c1', 'l1')
    expect(calls.map((call) => `${call.init?.method} ${call.url}`)).toEqual([
      'GET /api/chats/c1/invite-links',
      'POST /api/chats/c1/invite-links',
      'PUT /api/chats/c1/invite-links/l1',
      'POST /api/chats/c1/invite-links/l1/revoke',
      'POST /api/chats/c1/invite-links/primary',
      'GET /api/chats/c1/invite-links/l1/members',
      'GET /api/chats/c1/invite-links/l1/requests',
    ])
    expect(JSON.parse(String(calls[1]?.init?.body))).toEqual(input)
    expect((calls[0]?.init?.headers as Record<string, string>).Authorization).toBe('Bearer tok')
  })

  test('delete tolerates an empty 204', async () => {
    const { calls, client } = fakeClient(null, 204)
    await createInviteLinksApi(client, () => 'tok').remove('c1', 'l1')
    expect(calls[0]?.init?.method).toBe('DELETE')
  })

  test('the holder side encodes the token', async () => {
    const { calls, client } = fakeClient({ status: 'pending', chat_id: null })
    const api = createInviteLinksApi(client, () => 'tok')
    await api.preview('a_b-c')
    expect(await api.join('a_b-c')).toEqual({ status: 'pending', chat_id: null })
    expect(calls.map((call) => `${call.init?.method} ${call.url}`)).toEqual([
      'GET /api/invite-links/a_b-c',
      'POST /api/invite-links/a_b-c/join',
    ])
  })

  test('refusals map from the server error field', async () => {
    const { client } = fakeClient({ error: 'limit_reached' }, 410)
    const api = createInviteLinksApi(client, () => 'tok')
    const failure = await api.join('x').catch((error: unknown) => error)
    expect(failure).toBeInstanceOf(ApiError)
    expect(inviteRefusal(failure)).toBe('limit_reached')
    expect(inviteRefusal(new ApiError(404, '/x', 'Not Found'))).toBe('not_found')
    expect(inviteRefusal(new Error('boom'))).toBe('internal')
  })

  test('link urls', () => {
    expect(inviteLinkPath('abc')).toBe('/joinchat/abc')
    expect(inviteLinkUrl('https://chat.example/', 'abc')).toBe('https://chat.example/joinchat/abc')
  })
})

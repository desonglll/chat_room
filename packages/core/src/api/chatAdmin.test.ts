import { describe, expect, test } from 'bun:test'
import { ADMIN_ASSIGNABLE_KEYS, createChatAdminApi, MEMBER_TOGGLEABLE_KEYS } from './chatAdmin'
import { createApiClient } from './http'

function fakeClient(body: unknown) {
  const calls: Array<{ url: string; init: RequestInit | undefined }> = []
  const client = createApiClient({
    fetchImpl: async (url, init) => {
      calls.push({ url, init })
      return Response.json(body)
    },
  })
  return { calls, client }
}

const bodyOf = (init: RequestInit | undefined) => JSON.parse(String(init?.body))

describe('chat admin api', () => {
  test('reads hit the frozen paths with the bearer token', async () => {
    const { calls, client } = fakeClient({ items: [], next_cursor: null })
    const api = createChatAdminApi(client, () => 'tok')
    await api.permissions('c1')
    await api.member('c1', 'u1')
    await api.memberPage('c1')
    await api.memberPage('c1', { cursor: '17.abc', limit: 50, filter: 'admins' })
    expect(calls.map((call) => call.url)).toEqual([
      '/api/chats/c1/permissions',
      '/api/chats/c1/members/u1',
      '/api/chats/c1/members/page',
      '/api/chats/c1/members/page?cursor=17.abc&limit=50&filter=admins',
    ])
    expect((calls[0]?.init?.headers as Record<string, string>).Authorization).toBe('Bearer tok')
  })

  test('writes send the frozen bodies', async () => {
    const { calls, client } = fakeClient({})
    const api = createChatAdminApi(client, () => 'tok')
    await api.setDefaultPermissions('c1', ['message.send'])
    await api.appointAdmin('c1', 'u1', ['members.ban'], '版主')
    await api.dismissAdmin('c1', 'u1')
    await api.restrictMember('c1', 'u1', ['message.send'], '2026-10-02T00:00:00Z')
    expect(calls.map((call) => `${call.init?.method} ${call.url}`)).toEqual([
      'PUT /api/chats/c1/default-permissions',
      'PUT /api/chats/c1/members/u1/admin',
      'DELETE /api/chats/c1/members/u1/admin',
      'PUT /api/chats/c1/members/u1/restrictions',
    ])
    expect(bodyOf(calls[0]?.init)).toEqual({ permissions: ['message.send'] })
    expect(bodyOf(calls[1]?.init)).toEqual({ permissions: ['members.ban'], custom_title: '版主' })
    expect(bodyOf(calls[3]?.init)).toEqual({
      denied_permissions: ['message.send'],
      until: '2026-10-02T00:00:00Z',
    })
  })

  test('the key lists mirror the server registry sizes', () => {
    expect(MEMBER_TOGGLEABLE_KEYS).toHaveLength(9)
    expect(ADMIN_ASSIGNABLE_KEYS).toHaveLength(14)
    expect(ADMIN_ASSIGNABLE_KEYS).not.toContain('room.delete')
  })
})

import { describe, expect, test } from 'bun:test'
import { createApiClient } from './http'
import { createPrivacyApi, privacyRuleWrite, type PrivacyRule } from './privacy'

const user = (id: string) => ({ id, username: id, avatar_emoji: '', display_name: '' })

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

describe('privacy api', () => {
  test('GET and PUT hit the frozen paths with the bearer token', async () => {
    const { calls, client } = fakeClient({ rules: [] })
    const api = createPrivacyApi(client, () => 'tok')
    await api.get()
    await api.put('last_seen', { tier: 'nobody', allow_user_ids: ['a'], deny_user_ids: [] })
    expect(calls[0]?.url).toBe('/api/users/me/privacy')
    expect(calls[1]?.url).toBe('/api/users/me/privacy/last_seen')
    expect(calls[1]?.init?.method).toBe('PUT')
    expect(JSON.parse(String(calls[1]?.init?.body))).toEqual({
      tier: 'nobody',
      allow_user_ids: ['a'],
      deny_user_ids: [],
    })
    expect((calls[0]?.init?.headers as Record<string, string>).Authorization).toBe('Bearer tok')
  })

  test('search trims the query and bounds the result count', async () => {
    const { calls, client } = fakeClient([])
    await createPrivacyApi(client, () => null).searchUsers('  ann ')
    expect(calls[0]?.url).toBe('/api/users/search?q=ann&limit=20')
    expect((calls[0]?.init?.headers as Record<string, string>).Authorization).toBeUndefined()
  })

  test('a write drops the exception list the tier makes meaningless', () => {
    const rule = (tier: PrivacyRule['tier']): PrivacyRule => ({
      key: 'forwards',
      tier,
      allow_users: [user('a')],
      deny_users: [user('d')],
    })
    expect(privacyRuleWrite(rule('everybody'))).toEqual({ tier: 'everybody', allow_user_ids: [], deny_user_ids: ['d'] })
    expect(privacyRuleWrite(rule('contacts'))).toEqual({
      tier: 'contacts',
      allow_user_ids: ['a'],
      deny_user_ids: ['d'],
    })
    expect(privacyRuleWrite(rule('nobody'))).toEqual({ tier: 'nobody', allow_user_ids: ['a'], deny_user_ids: [] })
  })
})

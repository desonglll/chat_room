import { describe, expect, test } from 'bun:test'
import { clearEmojiStatus, getEmojiStatuses, resolveCustomEmoji, setEmojiStatus } from './customEmoji'
import { createApiClient } from './http'

function recorder(body: unknown = []) {
  const calls: Array<{ url: string; init: RequestInit | undefined }> = []
  const client = createApiClient({
    fetchImpl: async (url, init) => {
      calls.push({ url, init })
      return init?.method === 'DELETE' ? new Response(null, { status: 204 }) : Response.json(body)
    },
  })
  return { calls, client }
}

describe('custom emoji api', () => {
  test('resolves ids as one comma-separated query and skips empty lookups', async () => {
    const { calls, client } = recorder()
    expect(await resolveCustomEmoji(client, 't', [])).toEqual([])
    expect(await getEmojiStatuses(client, 't', [])).toEqual([])
    expect(calls).toHaveLength(0)
    await resolveCustomEmoji(client, 't', ['a', 'b'])
    await getEmojiStatuses(client, 't', ['u1'])
    expect(calls.map((call) => call.url)).toEqual(['/api/custom-emoji?ids=a%2Cb', '/api/users/emoji-statuses?ids=u1'])
  })

  test('sets and clears the emoji status on /api/users/me', async () => {
    const { calls, client } = recorder({ user_id: 'u' })
    await setEmojiStatus(client, 't', { custom_emoji_id: 'e', expires_at: null })
    await clearEmojiStatus(client, 't')
    expect(calls.map((call) => `${call.init?.method} ${call.url}`)).toEqual([
      'PUT /api/users/me/emoji-status',
      'DELETE /api/users/me/emoji-status',
    ])
    expect(JSON.parse(String(calls[0]!.init?.body))).toEqual({ custom_emoji_id: 'e', expires_at: null })
  })
})

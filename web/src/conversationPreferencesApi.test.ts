import { afterEach, describe, expect, it, mock } from 'bun:test'
import { updateConversationPreferences } from './conversationPreferencesApi'

const originalFetch = globalThis.fetch
afterEach(() => {
  globalThis.fetch = originalFetch
})

describe('conversation preferences API', () => {
  it('encodes the room id and sends an authenticated partial update', async () => {
    const fetchMock = mock(async (_input: RequestInfo | URL, _init?: RequestInit) =>
      Response.json({
        room_id: 'room/one',
        is_pinned: true,
        is_archived: false,
        notification_level: 'all',
        muted_until: null,
        updated_at: '2026-08-20T08:00:00Z',
      }),
    )
    globalThis.fetch = fetchMock as typeof fetch

    const result = await updateConversationPreferences('room/one', { is_pinned: true }, 'secret')

    expect(result.is_pinned).toBe(true)
    const [url, init] = fetchMock.mock.calls[0]!
    expect(String(url)).toContain('/api/conversations/room%2Fone/preferences')
    expect(init?.method).toBe('PATCH')
    expect(init?.headers).toMatchObject({ Authorization: 'Bearer secret' })
    expect(init?.body).toBe('{"is_pinned":true}')
  })

  it('maps missing conversations to an actionable message', async () => {
    globalThis.fetch = mock(async () => new Response(null, { status: 404 })) as typeof fetch

    await expect(updateConversationPreferences('missing', { is_archived: true }, 'token')).rejects.toThrow(
      '会话已失效，请刷新后重试',
    )
  })
})

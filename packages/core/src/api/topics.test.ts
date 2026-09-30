// TG-204: the forum topics client against an injected fake fetch, plus the pure helpers.
import { describe, expect, test } from 'bun:test'
import { createApiClient } from './http'
import type { ForumTopic } from './topics'
import { createTopicsApi, messageInTopic, sortTopics, TOPIC_COLORS, topicColorHex } from './topics'

function fakeClient(status = 200, body: unknown = {}) {
  const calls: Array<{ url: string; init: RequestInit | undefined }> = []
  const client = createApiClient({
    fetchImpl: async (url, init) => {
      calls.push({ url, init })
      return status === 204 ? new Response(null, { status }) : Response.json(body, { status })
    },
  })
  return { calls, api: createTopicsApi(client, () => 'tok') }
}

const bodyOf = (init: RequestInit | undefined) => JSON.parse(String(init?.body)) as unknown
const line = (call: { url: string; init: RequestInit | undefined }) => `${call.init?.method} ${call.url}`

const topic = (id: string, extra: Partial<ForumTopic> = {}): ForumTopic => ({
  id,
  chat_id: 'c1',
  is_general: false,
  title: id,
  icon_emoji: '',
  icon_custom_emoji_id: null,
  icon_color: TOPIC_COLORS[0],
  is_pinned: false,
  pinned_at: null,
  is_closed: false,
  is_hidden: false,
  creator_id: null,
  created_at: '2026-10-01T00:00:00Z',
  last_message: null,
  unread_count: 0,
  muted: false,
  muted_until: null,
  can_edit: false,
  ...extra,
})

describe('topics api', () => {
  test('list, create, update, forum toggle and notifications hit the frozen paths', async () => {
    const { calls, api } = fakeClient()
    await api.list('c/1')
    await api.create('c1', { title: '公告', icon_color: TOPIC_COLORS[2] })
    await api.update('c1', 't1', { is_closed: true, icon_custom_emoji_id: null })
    await api.setForum('c1', true)
    await api.setNotifications('c1', 't1', { muted: true, muted_until: null })
    await api.read('c1', 't1', 'm9')
    expect(calls.map(line)).toEqual([
      'GET /api/chats/c%2F1/topics',
      'POST /api/chats/c1/topics',
      'PATCH /api/chats/c1/topics/t1',
      'PUT /api/chats/c1/forum',
      'PUT /api/chats/c1/topics/t1/notifications',
      'POST /api/chats/c1/topics/t1/read',
    ])
    expect(bodyOf(calls[1]!.init)).toEqual({ title: '公告', icon_color: 0xcb86db })
    expect(bodyOf(calls[2]!.init)).toEqual({ is_closed: true, icon_custom_emoji_id: null })
    expect(bodyOf(calls[3]!.init)).toEqual({ enabled: true })
    expect(bodyOf(calls[4]!.init)).toEqual({ muted: true, muted_until: null })
    expect(bodyOf(calls[5]!.init)).toEqual({ message_id: 'm9' })
    expect((calls[0]!.init?.headers as Record<string, string>).Authorization).toBe('Bearer tok')
  })

  test('history pages use the topic endpoints with the /messages cursor', async () => {
    const { calls, api } = fakeClient(200, [])
    await api.messages('c1', 't1')
    await api.messages('c1', 't1', { before: 'm5', limit: 20 })
    await api.context('c1', 't1', 'm5', 30)
    expect(calls.map((call) => call.url)).toEqual([
      '/api/chats/c1/topics/t1/messages?limit=50',
      '/api/chats/c1/topics/t1/messages?limit=20&before=m5',
      '/api/chats/c1/topics/t1/messages/m5/context?limit=30',
    ])
  })

  test('a gone topic reads as null / an empty context; delete accepts 204', async () => {
    const missing = fakeClient(404)
    expect(await missing.api.get('c1', 't1')).toBeNull()
    expect(await missing.api.context('c1', 't1', 'm1')).toEqual([])
    const deleted = fakeClient(204)
    await deleted.api.remove('c1', 't1')
    expect(line(deleted.calls[0]!)).toBe('DELETE /api/chats/c1/topics/t1')
  })

  test('a refused create surfaces the status', async () => {
    const { api } = fakeClient(403)
    const error = (await api.create('c1', { title: 'x' }).catch((caught: unknown) => caught)) as { status: number }
    expect(error.status).toBe(403)
  })
})

describe('topic helpers', () => {
  test('a null or absent topic_id belongs to General only', () => {
    const general = topic('g', { is_general: true })
    const other = topic('t1')
    expect(messageInTopic({}, general)).toBeTrue()
    expect(messageInTopic({ topic_id: null }, general)).toBeTrue()
    expect(messageInTopic({ topic_id: null }, other)).toBeFalse()
    expect(messageInTopic({ topic_id: 't1' }, other)).toBeTrue()
    expect(messageInTopic({ topic_id: 't1' }, general)).toBeFalse()
  })

  test('colour hex is zero-padded and falls back to the palette', () => {
    expect(topicColorHex(0x6fb9f0)).toBe('#6fb9f0')
    expect(topicColorHex(0x00ff00)).toBe('#00ff00')
    expect(topicColorHex(-1)).toBe('#6fb9f0')
  })

  test('sort: pinned by pin time, then General, then newest activity', () => {
    const sorted = sortTopics([
      topic('old', { created_at: '2026-01-01T00:00:00Z' }),
      topic('g', { is_general: true }),
      topic('p2', { is_pinned: true, pinned_at: '2026-10-01T02:00:00Z' }),
      topic('busy', {
        created_at: '2025-01-01T00:00:00Z',
        last_message: { message_id: 'm', sender: 's', content: 'c', created_at: '2026-10-01T05:00:00Z' },
      }),
      topic('p1', { is_pinned: true, pinned_at: '2026-10-01T01:00:00Z' }),
    ])
    expect(sorted.map((row) => row.id)).toEqual(['p1', 'p2', 'g', 'busy', 'old'])
  })
})

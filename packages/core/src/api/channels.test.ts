import { describe, expect, test } from 'bun:test'
import { createChannelApi, MAX_VIEWED_POSTS } from './channels'
import { createApiClient } from './http'

function fakeClient(body: unknown, status = 200) {
  const calls: Array<{ url: string; init: RequestInit | undefined }> = []
  const client = createApiClient({
    fetchImpl: async (url, init) => {
      calls.push({ url, init })
      return Response.json(body, { status })
    },
  })
  return { calls, client }
}

const bodyOf = (init: RequestInit | undefined) => JSON.parse(String(init?.body))

describe('channel api', () => {
  test('create posts a channel descriptor to the canonical path', async () => {
    const { calls, client } = fakeClient({ id: 'c1' })
    await createChannelApi(client, () => 'tok').create({
      title: '  新闻  ',
      description: ' 每日 ',
      signaturesEnabled: true,
    })
    expect(calls[0]?.url).toBe('/api/chats')
    expect(bodyOf(calls[0]?.init)).toEqual({
      title: '新闻',
      password: null,
      join_policy: 'open',
      chat_type: 'channel',
      signatures_enabled: true,
      description: '每日',
    })
  })

  test('subscribe tells an approval wait from an active subscription', async () => {
    const pending = fakeClient({ status: 'pending' }, 202)
    const answer = await createChannelApi(pending.client, () => 'tok').subscribe('c1')
    expect(answer.state).toBe('pending')
    expect(pending.calls[0]?.url).toBe('/api/chats/c1/subscription')
    expect(pending.calls[0]?.init?.method).toBe('POST')
    const active = fakeClient({ status: 'active' })
    expect((await createChannelApi(active.client, () => 'tok').subscribe('c1', 'pw')).state).toBe('active')
    expect(bodyOf(active.calls[0]?.init)).toEqual({ password: 'pw' })
  })

  test('unsubscribe, signatures and view reports hit the frozen paths', async () => {
    const { calls, client } = fakeClient({ views: [{ message_id: 'm', views: 3 }] })
    const api = createChannelApi(client, () => 'tok')
    await api.unsubscribe('c1')
    await api.setSignatures('c1', false)
    const ids = Array.from({ length: MAX_VIEWED_POSTS + 5 }, (_, index) => `m${index}`)
    expect(await api.reportViews('c1', ids)).toEqual([{ message_id: 'm', views: 3 }])
    expect(calls.map((call) => `${call.init?.method} ${call.url}`)).toEqual([
      'DELETE /api/chats/c1/subscription',
      'PATCH /api/chats/c1/channel',
      'POST /api/chats/c1/message-views',
    ])
    expect(bodyOf(calls[1]?.init)).toEqual({ signatures_enabled: false })
    expect(bodyOf(calls[2]?.init).message_ids).toHaveLength(MAX_VIEWED_POSTS)
  })
})

/** Chat-list data flow against a fake fetch: canonical dialect only. */
import { expect, test } from 'bun:test'
import type { FetchLike } from '@tg/core'
import { createApiClient, createChatListStore } from '@tg/core'
import { createNewChat, loadChats } from '../src/features/chatList/chatListController'

const chatRow = (id: string, title: string) => ({
  id,
  chat_type: 'group',
  title,
  has_password: false,
  creator_user_id: 'u1',
  join_policy: 'open',
  avatar_emoji: '',
  description: '',
  username: null,
  is_forum: false,
  linked_chat_id: null,
  slow_mode_seconds: 0,
  auto_delete_seconds: 0,
  signatures_enabled: false,
  history_visible_to_new_members: true,
  member_count: 1,
  unread_count: 0,
  created_at: '2026-09-30T00:00:00Z',
})

function clientOf(handler: (url: string, init?: RequestInit) => Response) {
  const calls: Array<{ url: string; init?: RequestInit }> = []
  const fetchImpl: FetchLike = async (url, init) => {
    calls.push({ url, ...(init ? { init } : {}) })
    return handler(url, init)
  }
  return { client: createApiClient({ fetchImpl }), calls }
}

test('loadChats fills the store from GET /api/chats and clears the loading flag', async () => {
  const store = createChatListStore()
  const { client, calls } = clientOf(() => Response.json([chatRow('c1', '甲'), chatRow('c2', '乙')]))
  await loadChats({ client, token: 'tok', store })
  expect(calls[0]?.url).toBe('/api/chats')
  expect(store.getState().chats.map((chat) => chat.title)).toEqual(['甲', '乙'])
  expect(store.getState().loading).toBeFalse()
  expect(store.getState().error).toBe('')
})

test('a failed load records a status instead of throwing at the caller', async () => {
  const store = createChatListStore()
  const { client } = clientOf(() => Response.json({ error: 'x' }, { status: 500 }))
  await loadChats({ client, token: 'tok', store })
  expect(store.getState().error).toBe('500')
  expect(store.getState().loading).toBeFalse()
})

test('createNewChat POSTs the canonical open-chat body and upserts the result', async () => {
  const store = createChatListStore()
  const { client, calls } = clientOf(() => Response.json(chatRow('c9', '新群')))
  const chat = await createNewChat({ client, token: 'tok', store }, '  新群  ')
  expect(chat.id).toBe('c9')
  expect(calls[0]?.url).toBe('/api/chats')
  const body = JSON.parse(String(calls[0]?.init?.body)) as Record<string, unknown>
  // `title` (canonical, never the alias `name`), passwordless, open — TG-012 decision.
  expect(body).toEqual({ title: '新群', password: null, join_policy: 'open' })
  expect(store.getState().chats.map((row) => row.id)).toContain('c9')
})

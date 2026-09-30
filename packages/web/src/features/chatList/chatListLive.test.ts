/** Live list updates: store reducers, the account socket feed, the timeline mirror. */
import { describe, expect, test } from 'bun:test'
import type { AccountMessageEvent, CoreClock, CoreSocket, DisplayMessage } from '@tg/core'
import { createChatListStore, createMessageStore } from '@tg/core'
import { startAccountFeed } from './accountFeed'
import { conversation } from './chatListFixtures'
import { latestSettledMessage, mirrorTimelines } from './timelineMirror'

const event = (overrides: Partial<AccountMessageEvent> = {}): AccountMessageEvent => ({
  type: 'new_message',
  message_id: 'm2',
  room_id: 'a',
  conversation_kind: 'group',
  conversation_title: 'A',
  sender_id: 'u2',
  sender: 'alice',
  content: 'hi',
  attachment_file_name: null,
  timestamp: '2026-10-01T10:00:00Z',
  is_mention: false,
  ...overrides,
})

describe('chatListStore live reducers', () => {
  test('applyAccountMessage sets the preview and counts unread except for the open chat', () => {
    const store = createChatListStore()
    store.getState().setConversations([conversation('a'), conversation('b')])
    store.getState().applyAccountMessage(event(), '')
    store.getState().applyAccountMessage(event({ message_id: 'm3', timestamp: '2026-10-01T10:01:00Z' }), '')
    store.getState().applyAccountMessage(event({ room_id: 'b' }), 'b')
    const [a, b] = store.getState().conversations
    expect(a?.unread_count).toBe(2)
    expect(a?.last_message?.message_id).toBe('m3')
    expect(a?.last_activity_at).toBe('2026-10-01T10:01:00Z')
    expect(b?.unread_count).toBe(0)
  })

  test('a duplicate or older event changes nothing', () => {
    const store = createChatListStore()
    store.getState().setConversations([conversation('a')])
    store.getState().applyAccountMessage(event(), '')
    store.getState().applyAccountMessage(event(), '')
    store.getState().applyAccountMessage(event({ message_id: 'old', timestamp: '2026-09-01T00:00:00Z' }), '')
    expect(store.getState().conversations[0]).toMatchObject({ unread_count: 1, last_message: { message_id: 'm2' } })
  })

  test('applyUnreadCounts overwrites counts and drops chats no longer active', () => {
    const store = createChatListStore()
    store.getState().setConversations([conversation('a'), conversation('b'), conversation('c')])
    store.getState().applyUnreadCounts([
      { room_id: 'a', unread_count: 7, membership_status: 'active' },
      { room_id: 'b', unread_count: 0, membership_status: 'banned' },
    ])
    expect(store.getState().conversations.map((row) => [row.room_id, row.unread_count])).toEqual([
      ['a', 7],
      ['c', 0],
    ])
  })

  test('applyLatestMessage replaces a recalled same message and ignores older ones, unread untouched', () => {
    const store = createChatListStore()
    store.getState().setConversations([conversation('a', { unread_count: 2 })])
    const latest = {
      message_id: 'm1',
      sender_id: 'me',
      sender: 'me',
      content: 'mine',
      attachment_file_name: null,
      recalled: false,
      created_at: '2026-10-01T10:00:00Z',
    }
    store.getState().applyLatestMessage('a', latest)
    store.getState().applyLatestMessage('a', { ...latest, recalled: true })
    store.getState().applyLatestMessage('a', { ...latest, message_id: 'm0', created_at: '2026-09-01T00:00:00Z' })
    expect(store.getState().conversations[0]).toMatchObject({
      unread_count: 2,
      last_message: { message_id: 'm1', recalled: true },
    })
  })

  test('setUnreadCount and removeChat reach the sidebar rows too', () => {
    const store = createChatListStore()
    store.getState().setConversations([conversation('a'), conversation('b')])
    store.getState().setUnreadCount('a', 5)
    store.getState().removeChat('b')
    expect(store.getState().conversations.map((row) => [row.room_id, row.unread_count])).toEqual([['a', 5]])
  })
})

function fakeSocketWorld() {
  const sockets: Array<CoreSocket & { sent: string[]; closed: boolean }> = []
  const timers: Array<{ callback: () => void; delay: number; cleared: boolean }> = []
  const clock: CoreClock = {
    now: () => 0,
    setTimeout: (callback, delay) => {
      const timer = { callback, delay, cleared: false }
      timers.push(timer)
      return timer
    },
    clearTimeout: (handle) => {
      ;(handle as { cleared: boolean }).cleared = true
    },
  }
  const factory = () => {
    const socket = {
      sent: [] as string[],
      closed: false,
      send(data: string) {
        socket.sent.push(data)
      },
      close() {
        socket.closed = true
      },
      onopen: null as (() => void) | null,
      onmessage: null as ((data: string) => void) | null,
      onclose: null as ((info: { code: number; reason: string }) => void) | null,
      onerror: null as (() => void) | null,
    }
    sockets.push(socket)
    return socket
  }
  return { sockets, timers, clock, factory }
}

describe('startAccountFeed', () => {
  test('authenticates, applies frames, resyncs on unknown chats, reconnects with backoff', () => {
    const world = fakeSocketWorld()
    const store = createChatListStore()
    store.getState().setConversations([conversation('a')])
    let resyncs = 0
    const stop = startAccountFeed({
      url: 'ws://x/ws/account',
      token: 'tok',
      socketFactory: world.factory,
      clock: world.clock,
      store,
      activeChatId: () => '',
      resync: () => {
        resyncs += 1
      },
    })
    const first = world.sockets[0]!
    first.onopen?.()
    expect(first.sent).toEqual([JSON.stringify({ token: 'tok' })])
    expect(resyncs).toBe(0)

    first.onmessage?.(JSON.stringify(event()))
    expect(store.getState().conversations[0]?.unread_count).toBe(1)
    first.onmessage?.(JSON.stringify(event({ room_id: 'new-chat' })))
    expect(resyncs).toBe(1)
    first.onmessage?.(
      JSON.stringify({
        type: 'unread_counts',
        chats: [{ room_id: 'a', unread_count: 9, membership_status: 'active' }],
      }),
    )
    expect(store.getState().conversations[0]?.unread_count).toBe(9)
    first.onmessage?.('not json')
    first.onmessage?.(JSON.stringify({ type: 'social_changed', incoming_request_count: 1 }))

    first.onclose?.({ code: 1006, reason: '' })
    expect(world.timers[0]?.delay).toBe(500)
    world.timers[0]?.callback()
    const second = world.sockets[1]!
    second.onopen?.()
    expect(resyncs).toBe(2) // missed events while offline

    stop()
    expect(second.closed).toBe(true)
    second.onclose?.({ code: 1000, reason: '' })
    expect(world.timers).toHaveLength(1)
  })
})

describe('timeline mirror', () => {
  const broadcast = (id: string, overrides: Record<string, unknown> = {}): DisplayMessage =>
    ({
      type: 'broadcast',
      message_id: id,
      sender_id: 'me',
      sender: 'me',
      sender_avatar: '',
      content: `text ${id}`,
      attachment: null,
      reply_to: null,
      recalled_at: null,
      edited_at: null,
      timestamp: '2026-10-01T10:00:00Z',
      favorite_id: null,
      forwarded_from: null,
      reactions: [],
      ...overrides,
    }) as DisplayMessage

  test('latestSettledMessage skips pending sends and non-broadcast rows', () => {
    const messages = [
      broadcast('m1'),
      broadcast('m2', { delivery_state: 'sending' }),
      { type: 'system', key: 's', content: 'x' } as DisplayMessage,
    ]
    expect(latestSettledMessage(messages)?.message_id).toBe('m1')
    expect(latestSettledMessage([])).toBeNull()
  })

  test('mirrorTimelines pushes a changed timeline into the sidebar row', () => {
    const messages = createMessageStore()
    const list = createChatListStore()
    list.getState().setConversations([conversation('a')])
    const stop = mirrorTimelines(messages, list)
    messages.getState().applyBroadcast('a', broadcast('m9') as never, 'none')
    expect(list.getState().conversations[0]?.last_message).toMatchObject({ message_id: 'm9', content: 'text m9' })
    stop()
  })
})

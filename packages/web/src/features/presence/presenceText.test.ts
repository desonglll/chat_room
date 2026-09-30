// TG-107: the web-side derivations — header status per chat type, and the end-to-end
// "typing line gone within 5 s of the last frame" path through store + shared ticker.
import { describe, expect, test } from 'bun:test'
import type { Chat, ChatType } from '@tg/core'
import { createChatListStore, createPresenceStore } from '@tg/core'
import { createNowTicker } from './nowTicker'
import type { PresenceInputs } from './presenceText'
import { headerStatusFor, memberCountText, nextTypingExpiry, typingSummaryFor } from './presenceText'
import { FakeClock } from './testClock'

const chat = (id: string, chat_type: ChatType, member_count = 3) =>
  ({ id, chat_type, title: id, member_count }) as unknown as Chat

function world(start = 100_000) {
  const clock = new FakeClock(start)
  const presence = createPresenceStore()
  const chatList = createChatListStore()
  chatList.getState().setChats([chat('g', 'group', 12), chat('p', 'private', 2), chat('ch', 'channel', 900)])
  const inputs = (): PresenceInputs => ({
    presence: presence.getState(),
    chatList: chatList.getState(),
    currentUserId: 'me',
    now: clock.now(),
  })
  const typing = (chatId: string, user_id: string, action: 'typing' | 'recording_voice' | 'cancel' = 'typing') =>
    presence
      .getState()
      .applyTyping(
        chatId,
        { type: 'typing', content: 'x', action, user_id, username: user_id.toUpperCase() },
        clock.now(),
      )
  return { clock, presence, chatList, inputs, typing }
}

describe('headerStatusFor', () => {
  test('typing wins; private omits the name; groups name the actor', () => {
    const w = world()
    w.typing('g', 'a', 'recording_voice')
    w.typing('p', 'peer', 'recording_voice')
    expect(headerStatusFor(w.inputs(), 'g')).toEqual({ kind: 'typing', text: 'A 正在录音', action: 'recording_voice' })
    expect(headerStatusFor(w.inputs(), 'p')).toEqual({ kind: 'typing', text: '正在录音', action: 'recording_voice' })
  })

  test('a stop frame clears immediately', () => {
    const w = world()
    w.typing('g', 'a')
    w.typing('g', 'a', 'cancel')
    expect(typingSummaryFor(w.inputs(), 'g')).toBeNull()
  })

  test('groups show member count and online count above one; channels show subscribers', () => {
    const w = world()
    expect(headerStatusFor(w.inputs(), 'g')).toEqual({ kind: 'members', text: '12 位成员' })
    w.presence.getState().applyPresence('g', {
      type: 'presence',
      members: [
        { user_id: 'me', username: 'me', avatar_emoji: '' },
        { user_id: 'a', username: 'a', avatar_emoji: '' },
      ],
      participants: [],
    })
    expect(headerStatusFor(w.inputs(), 'g')).toEqual({ kind: 'members', text: '12 位成员，2 人在线' })
    expect(headerStatusFor(w.inputs(), 'ch')).toEqual({ kind: 'members', text: '900 位订阅者' })
    expect(memberCountText('supergroup', 1, 1)).toBe('1 位成员')
  })

  test('private chats show the peer last-seen, updating as time passes', () => {
    const w = world(Date.UTC(2026, 8, 30, 12, 0))
    w.presence.getState().applyAuthOk('p', {
      type: 'auth_ok',
      room_name: 'p',
      members: [{ user_id: 'me', username: 'me', avatar_emoji: '' }],
      participants: [
        { user_id: 'me', username: 'me', avatar_emoji: '' },
        { user_id: 'peer', username: 'peer', avatar_emoji: '' },
      ],
      read_receipts: [],
      statuses: [{ user_id: 'peer', status: { kind: 'online' } }],
    })
    expect(headerStatusFor(w.inputs(), 'p')).toEqual({ kind: 'last_seen', text: '在线', online: true })
    w.presence.getState().applyUserStatus('p', {
      type: 'user_status',
      user_id: 'peer',
      status: { kind: 'offline', last_seen: new Date(w.clock.now()).toISOString() },
    })
    expect(headerStatusFor(w.inputs(), 'p')).toEqual({ kind: 'last_seen', text: '刚刚上线', online: false })
    w.clock.advance(3 * 60_000)
    expect(headerStatusFor(w.inputs(), 'p').text).toBe('3 分钟前上线')
  })
})

test('with no stop frame the typing line disappears within 5 s, woken by the shared ticker', () => {
  const w = world()
  const ticker = createNowTicker({
    clock: w.clock,
    periodMs: 60_000, // deliberately long: only the exact wake-up can clear it in time
    wakeSource: {
      subscribe: (listener) => w.presence.subscribe(listener),
      nextWakeAt: (now) => nextTypingExpiry(w.presence.getState(), now),
    },
  })
  const seen: Array<[number, string | null]> = []
  const start = w.clock.now()
  ticker.subscribe(() => seen.push([w.clock.now() - start, typingSummaryFor(w.inputs(), 'g')?.text ?? null]))
  w.typing('g', 'a')
  w.clock.advance(2_000)
  w.typing('g', 'b') // second actor refreshes nothing for A
  expect(typingSummaryFor(w.inputs(), 'g')?.text).toBe('A 和 B 正在输入')
  w.clock.advance(3_000)
  expect(seen).toEqual([[5_000, 'B 正在输入']])
  w.clock.advance(2_000)
  expect(seen.at(-1)).toEqual([7_000, null])
  expect(seen.every(([at]) => at <= 7_000)).toBe(true)
})

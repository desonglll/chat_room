import { describe, expect, test } from 'bun:test'
import { applyMessageReaction } from './messageReactions'
import type { BroadcastMessage, DisplayMessage } from './types'

const broadcast = (reactions: BroadcastMessage['reactions'] = []): BroadcastMessage => ({
  type: 'broadcast',
  message_id: 'message-1',
  sender_id: 'sender',
  sender: 'Sender',
  sender_avatar: '',
  content: 'hello',
  attachment: null,
  reply_to: null,
  recalled_at: null,
  edited_at: null,
  timestamp: '2026-08-20T00:00:00Z',
  forwarded_from: null,
  reactions,
})

describe('message reactions', () => {
  test('adds a reaction and keeps duplicate active events idempotent', () => {
    const initial: DisplayMessage[] = [broadcast()]
    const event = { message_id: 'message-1', emoji: '👍', user_id: 'alice', active: true }

    const once = applyMessageReaction(initial, event)
    const twice = applyMessageReaction(once, event)

    expect((twice[0] as BroadcastMessage).reactions).toEqual([{ emoji: '👍', user_ids: ['alice'] }])
  })

  test('aggregates users and removes an empty reaction', () => {
    const initial: DisplayMessage[] = [broadcast([{ emoji: '❤️', user_ids: ['alice', 'bob'] }])]
    const withoutAlice = applyMessageReaction(initial, {
      message_id: 'message-1',
      emoji: '❤️',
      user_id: 'alice',
      active: false,
    })
    const empty = applyMessageReaction(withoutAlice, {
      message_id: 'message-1',
      emoji: '❤️',
      user_id: 'bob',
      active: false,
    })

    expect((withoutAlice[0] as BroadcastMessage).reactions).toEqual([{ emoji: '❤️', user_ids: ['bob'] }])
    expect((empty[0] as BroadcastMessage).reactions).toEqual([])
  })

  test('leaves unrelated and non-message entries unchanged', () => {
    const system: DisplayMessage = { type: 'system', key: 'system-1', content: 'joined' }
    const initial: DisplayMessage[] = [broadcast(), system]
    const next = applyMessageReaction(initial, {
      message_id: 'missing',
      emoji: '😂',
      user_id: 'alice',
      active: true,
    })

    expect(next).toEqual(initial)
  })
})

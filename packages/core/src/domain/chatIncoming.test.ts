// Migrated from web/src/chatIncoming.test.ts (TG-011); only import paths changed.
import { describe, expect, test } from 'bun:test'
import { mergeIncomingBroadcast } from './chatIncoming'
import type { BroadcastMessage } from './messageView'

const message = (reactions: BroadcastMessage['reactions'] = []): BroadcastMessage => ({
  type: 'broadcast',
  message_id: 'message-1',
  sender_id: 'user-2',
  sender: 'friend',
  sender_avatar: '',
  content: 'hello',
  attachment: null,
  reply_to: null,
  recalled_at: null,
  favorite_id: null,
  edited_at: null,
  timestamp: '2026-08-20T00:00:00Z',
  forwarded_from: null,
  reactions,
})

describe('incoming chat messages', () => {
  test('replaces duplicate history with authoritative reaction state', () => {
    const result = mergeIncomingBroadcast([message()], message([{ emoji: '👍', user_ids: ['user-2'] }]), 'incoming')
    expect(result.messages).toHaveLength(1)
    expect((result.messages[0] as BroadcastMessage).reactions).toEqual([{ emoji: '👍', user_ids: ['user-2'] }])
  })

  test('classifies a new message with the supplied motion', () => {
    const result = mergeIncomingBroadcast([], message(), 'incoming')
    expect((result.messages[0] as BroadcastMessage).motion).toBe('incoming')
  })

  // TG-1208: a send made while the chat was opening sat ABOVE the replayed history.
  test('a confirmed row lands before trailing in-flight sends, after failed ones', () => {
    const pending = (id: string, state: 'sending' | 'failed'): BroadcastMessage => ({
      ...message(),
      message_id: `pending:${id}`,
      sender_id: 'me',
      content: id,
      delivery_state: state,
    })
    const replayed = { ...message(), message_id: 'history-1', content: 'history' }
    const inFlight = mergeIncomingBroadcast([pending('a', 'sending'), pending('b', 'sending')], replayed, 'none')
    expect(inFlight.messages.map((row) => (row as BroadcastMessage).content)).toEqual(['history', 'a', 'b'])
    const failed = mergeIncomingBroadcast([pending('x', 'failed')], replayed, 'none')
    expect(failed.messages.map((row) => (row as BroadcastMessage).content)).toEqual(['x', 'history'])
  })
})

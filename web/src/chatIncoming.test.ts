import { describe, expect, test } from 'bun:test'
import { mergeIncomingBroadcast } from './chatIncoming'
import type { BroadcastMessage } from './types'

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
})

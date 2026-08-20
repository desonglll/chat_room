import { describe, expect, test } from 'bun:test'
import { firstUnreadMessageId, messageStartScrollTop } from './messageViewportPolicy'

const message = (messageId: string, senderId: string | null) => ({
  message_id: messageId,
  sender_id: senderId,
})

describe('initial message viewport', () => {
  const messages = [
    message('read', 'other'),
    message('own', 'current'),
    message('unread-1', 'other'),
    message('unread-2', null),
  ]

  test('returns no target when the room is fully read', () => {
    expect(firstUnreadMessageId(messages, 0, 'current')).toBe('')
  })

  test('positions at the first genuinely unread incoming message', () => {
    expect(firstUnreadMessageId(messages, 2, 'current')).toBe('unread-1')
  })

  test('uses the earliest loaded incoming message when unread history is longer', () => {
    expect(firstUnreadMessageId(messages, 20, 'current')).toBe('read')
  })

  test('ignores recalled messages just like the server unread count', () => {
    const recalled = { ...message('recalled', 'other'), recalled_at: '2026-08-19T00:00:00Z' }
    expect(firstUnreadMessageId([...messages, recalled], 1, 'current')).toBe('unread-2')
  })

  test('positions an unread message inside its own scroller with breathing room', () => {
    expect(messageStartScrollTop({ containerTop: 168, currentScrollTop: 40, messageTop: 240 })).toBe(92)
  })

  test('never scrolls above the beginning of the message list', () => {
    expect(messageStartScrollTop({ containerTop: 168, currentScrollTop: 0, messageTop: 174 })).toBe(0)
  })
})

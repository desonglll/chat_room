// TG-100: older REST pages live in the store timeline, so live frames reach them, and
// read cursors drive the outgoing double tick. (The rest of the store: stores.test.ts.)
import { describe, expect, test } from 'bun:test'
import type { BroadcastMessage } from '../domain/messageView'
import { createMessageStore, peerReadThrough, selectTimeline } from './messageStore'

const CHAT = 'c1'

function message(id: string, minute: number, sender = 'u2'): BroadcastMessage {
  return {
    type: 'broadcast',
    message_id: id,
    sender_id: sender,
    sender: sender,
    sender_avatar: '',
    content: `text ${id}`,
    attachment: null,
    reply_to: null,
    recalled_at: null,
    edited_at: null,
    timestamp: new Date(Date.UTC(2026, 9, 1, 10, minute)).toISOString(),
    favorite_id: null,
    forwarded_from: null,
    reactions: [],
  }
}

function seeded() {
  const store = createMessageStore()
  store.getState().applyBroadcast(CHAT, message('m10', 10), 'none')
  store.getState().applyBroadcast(CHAT, message('m11', 11), 'none')
  return store
}

const ids = (store: ReturnType<typeof createMessageStore>) =>
  selectTimeline(CHAT)(store.getState()).messages.map((row) => (row.type === 'broadcast' ? row.message_id : row.key))

describe('messageStore.prependHistory', () => {
  test('puts an older page in front, chronologically, and reports the count', () => {
    const store = seeded()
    // REST pages come in any order and the `before` cursor is inclusive (m10 repeats).
    const added = store.getState().prependHistory(CHAT, [message('m10', 10), message('m8', 8), message('m9', 9)])
    expect(added).toBe(2)
    expect(ids(store)).toEqual(['m8', 'm9', 'm10', 'm11'])
  })

  test('a page that adds nothing keeps the timeline identity', () => {
    const store = seeded()
    const before = selectTimeline(CHAT)(store.getState())
    expect(store.getState().prependHistory(CHAT, [message('m10', 10), message('m11', 11)])).toBe(0)
    expect(selectTimeline(CHAT)(store.getState())).toBe(before)
  })

  test('never inserts inside or after the loaded range, and dedups within the page', () => {
    const store = seeded()
    const added = store.getState().prependHistory(CHAT, [message('m12', 12), message('m7', 7), message('m7', 7)])
    expect(added).toBe(1)
    expect(ids(store)).toEqual(['m7', 'm10', 'm11'])
  })

  test('pending optimistic rows do not bound the page', () => {
    const store = createMessageStore()
    store.getState().appendOptimistic(CHAT, {
      clientMessageId: 'cid',
      content: 'hi',
      replyTo: '',
      currentUserId: 'u1',
      participants: [],
    })
    expect(store.getState().prependHistory(CHAT, [message('m1', 1)])).toBe(1)
    expect(ids(store)[0]).toBe('m1')
  })

  test('edits, recalls and reactions reach rows that arrived through a REST page', () => {
    const store = seeded()
    store.getState().prependHistory(CHAT, [message('m5', 5)])
    store.getState().applyEdit(CHAT, { type: 'message_edited', message_id: 'm5', content: 'new', edited_at: 'e' })
    store.getState().applyReaction(CHAT, {
      type: 'reaction_changed',
      message_id: 'm5',
      emoji: '👍',
      user_id: 'u3',
      active: true,
    })
    let first = selectTimeline(CHAT)(store.getState()).messages[0] as BroadcastMessage
    expect(first.content).toBe('new')
    expect(first.edited_at).toBe('e')
    expect(first.reactions).toEqual([{ emoji: '👍', user_ids: ['u3'] }])
    store.getState().applyRecall(CHAT, { type: 'message_recalled', message_id: 'm5', recalled_at: 'r' })
    first = selectTimeline(CHAT)(store.getState()).messages[0] as BroadcastMessage
    expect(first.recalled_at).toBe('r')
  })
})

describe('messageStore read cursors', () => {
  test('peerReadThrough is the newest loaded message another member read', () => {
    const store = seeded()
    store.getState().applyBroadcast(CHAT, message('m12', 12, 'u1'), 'none')
    const timeline = () => selectTimeline(CHAT)(store.getState())
    expect(peerReadThrough(timeline(), 'u1')).toBe('')
    store.getState().applyReadReceipts(CHAT, [
      { user_id: 'u1', message_id: 'm12' }, // the viewer's own cursor never counts
      { user_id: 'u2', message_id: 'm10' },
    ])
    expect(peerReadThrough(timeline(), 'u1')).toBe(message('m10', 10).timestamp)
    store.getState().applyReadReceipts(CHAT, [{ user_id: 'u3', message_id: 'm12' }])
    expect(peerReadThrough(timeline(), 'u1')).toBe(message('m12', 12).timestamp)
  })

  test('a cursor outside the loaded window marks nothing read', () => {
    const store = seeded()
    store.getState().applyReadReceipts(CHAT, [{ user_id: 'u2', message_id: 'gone' }])
    expect(peerReadThrough(selectTimeline(CHAT)(store.getState()), 'u1')).toBe('')
  })
})

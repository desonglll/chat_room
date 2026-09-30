import { describe, expect, test } from 'bun:test'
import type { BroadcastMessage, DisplayMessage } from './messageView'
import { messageKey } from './messageViewportLayout'
import {
  broadcastIds,
  countUnreadBelow,
  mergeMessagePages,
  newestCursor,
  oldestCursor,
  overlapsLive,
  prependShift,
  splitContextWindow,
  withoutLive,
} from './messageViewportWindow'

const BASE = Date.parse('2026-09-30T10:00:00Z')

function msg(n: number, sender = 'a', extra: Partial<BroadcastMessage> = {}): BroadcastMessage {
  return {
    type: 'broadcast',
    message_id: `m${String(n).padStart(4, '0')}`,
    sender_id: sender,
    sender,
    sender_avatar: '',
    content: '',
    attachment: null,
    reply_to: null,
    recalled_at: null,
    edited_at: null,
    timestamp: new Date(BASE + n * 1000).toISOString(),
    favorite_id: null,
    forwarded_from: null,
    reactions: [],
    ...extra,
  }
}

const range = (from: number, to: number) => Array.from({ length: to - from + 1 }, (_, i) => msg(from + i))
const ids = (messages: BroadcastMessage[]) => messages.map((message) => message.message_id)

describe('mergeMessagePages', () => {
  test('an older page is prepended and the inclusive cursor duplicate dropped', () => {
    const existing = range(10, 12)
    const { messages, added } = mergeMessagePages(existing, range(5, 10))
    expect(added).toBe(5)
    expect(ids(messages)).toEqual(ids(range(5, 12)))
    expect(messages[5]).toBe(existing[0] as BroadcastMessage)
  })

  test('a newer page is appended', () => {
    const { messages, added } = mergeMessagePages(range(1, 3), range(3, 6))
    expect(added).toBe(3)
    expect(ids(messages)).toEqual(ids(range(1, 6)))
  })

  test('an overlapping unordered page is merged and sorted', () => {
    const { messages, added } = mergeMessagePages(range(1, 3).concat(range(7, 8)), [msg(5), msg(4), msg(6), msg(2)])
    expect(added).toBe(3)
    expect(ids(messages)).toEqual(ids(range(1, 8)))
  })

  test('an empty or fully known page adds nothing', () => {
    expect(mergeMessagePages(range(1, 3), []).added).toBe(0)
    expect(mergeMessagePages(range(1, 3), range(1, 2)).added).toBe(0)
  })

  test('equal timestamps order by id', () => {
    const tie = { timestamp: new Date(BASE).toISOString() }
    const { messages } = mergeMessagePages([msg(3, 'a', tie)], [msg(2, 'a', tie), msg(1, 'a', tie)])
    expect(ids(messages)).toEqual(['m0001', 'm0002', 'm0003'])
  })
})

describe('cursors', () => {
  const pending = msg(9, 'me', { message_id: 'pending:x' })
  test('pending optimistic rows never become cursors', () => {
    expect(oldestCursor([pending, msg(1), msg(2)])).toBe('m0001')
    expect(newestCursor([msg(1), msg(2), pending])).toBe('m0002')
    expect(oldestCursor([pending])).toBe('')
    expect(newestCursor([])).toBe('')
  })

  test('system rows are skipped', () => {
    const rows: DisplayMessage[] = [{ type: 'system', key: 's', content: '' }, msg(3)]
    expect(oldestCursor(rows)).toBe('m0003')
    expect([...broadcastIds(rows)]).toEqual(['m0003'])
  })
})

describe('splitContextWindow', () => {
  test('a full window has more on both sides', () => {
    // limit 10: up to 6 older (incl. target) + 4 newer.
    expect(splitContextWindow(range(1, 10), 'm0006', 10)).toEqual({
      targetIndex: 5,
      reachedStart: false,
      reachedEnd: false,
    })
  })

  test('a short older side means the chat start is loaded', () => {
    expect(splitContextWindow(range(1, 10), 'm0003', 10).reachedStart).toBe(true)
  })

  test('a short newer side means the newest message is loaded', () => {
    expect(splitContextWindow(range(1, 8), 'm0006', 10).reachedEnd).toBe(true)
  })

  test('a missing target reports -1', () => {
    expect(splitContextWindow(range(1, 3), 'nope', 10).targetIndex).toBe(-1)
  })
})

describe('live joins', () => {
  test('overlap and subtraction by id', () => {
    const live = broadcastIds(range(8, 12))
    expect(overlapsLive(range(1, 7), live)).toBe(false)
    expect(overlapsLive(range(1, 8), live)).toBe(true)
    expect(ids(withoutLive(range(6, 9), live))).toEqual(['m0006', 'm0007'])
  })
})

describe('prependShift', () => {
  test('counts rows inserted before the previous first row', () => {
    expect(prependShift('c', ['a', 'b', 'c', 'd'])).toBe(2)
    expect(prependShift('a', ['a', 'b'])).toBe(0)
    expect(prependShift('', ['a'])).toBe(0)
    expect(prependShift('gone', ['a'])).toBe(0)
  })
})

describe('countUnreadBelow', () => {
  const rows: DisplayMessage[] = [msg(1), msg(2, 'me'), msg(3), msg(4, 'b', { recalled_at: 'x' }), msg(5)]
  test('counts incoming, non-recalled rows after the seen row', () => {
    expect(countUnreadBelow(rows, 'm0001', 'me', messageKey)).toBe(2)
    expect(countUnreadBelow(rows, 'm0005', 'me', messageKey)).toBe(0)
  })

  test('nothing seen yet means nothing to announce', () => {
    expect(countUnreadBelow(rows, '', 'me', messageKey)).toBe(0)
  })
})

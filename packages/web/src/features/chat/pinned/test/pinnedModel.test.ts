/** TG-901: the pinned bar shows the newest pin first, cycles older on click, skips recalled. */
import { expect, test } from 'bun:test'
import type { StoredMessage } from '@tg/core'
import type { ChatPin } from '../pinnedApi'
import { nextPinIndex, pinPreview, toPinEntries } from '../pinnedModel'

const message = (id: string, content: string, extra: Partial<StoredMessage> = {}): StoredMessage => ({
  id,
  room_id: 'c',
  client_message_id: null,
  sender_id: 'u',
  sender: 'a',
  sender_avatar: '',
  content,
  attachment: null,
  reply_to: null,
  recalled_at: null,
  edited_at: null,
  created_at: '2026-10-01T00:00:00Z',
  favorite_id: null,
  forwarded_from: null,
  reactions: [],
  ...extra,
})
const pin = (m: StoredMessage, at: string): ChatPin => ({ message: m, pinned_by: 'u', pinned_at: at })

test('newest first, recalled dropped, text collapsed to one line', () => {
  const entries = toPinEntries([
    pin(message('old', 'first\n  line'), '2026-10-01T01:00:00Z'),
    pin(message('new', 'second'), '2026-10-01T03:00:00Z'),
    pin(message('gone', 'x', { recalled_at: '2026-10-01T04:00:00Z' }), '2026-10-01T02:00:00Z'),
  ])
  expect(entries.map((entry) => [entry.messageId, entry.text])).toEqual([
    ['new', 'second'],
    ['old', 'first line'],
  ])
})

test('a click moves to the next older pin and wraps', () => {
  expect(nextPinIndex(0, 3)).toBe(1)
  expect(nextPinIndex(2, 3)).toBe(0)
  expect(nextPinIndex(0, 0)).toBe(0)
})

test('media pins describe themselves', () => {
  const file = {
    id: 'a',
    file_name: 'plan.pdf',
    mime_type: 'application/pdf',
    size_bytes: 1,
    download_url: '',
    is_sensitive: false,
  }
  expect(pinPreview(message('f', '', { attachment: file }))).toBe('plan.pdf')
  expect(pinPreview(message('p', '', { poll: {} as never }))).toBe('投票')
})

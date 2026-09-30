import { expect, test } from 'bun:test'
import type { BroadcastMessage, DisplayMessage } from '@tg/core'
import { compareItems, mediaFromMessages, mergeItems, viewerKind } from '../mediaItem'
import { toMediaPage } from '../chatMediaSource'
import { fileRow, media } from './fixtures'

function broadcast(n: number, mime: string, extra: Partial<BroadcastMessage> = {}): BroadcastMessage {
  const row = fileRow(n, mime)
  return {
    type: 'broadcast',
    message_id: row.message_id,
    sender_id: 'u1',
    sender: 'alice',
    sender_avatar: '',
    content: `text ${n}`,
    attachment: row.attachment,
    reply_to: null,
    recalled_at: null,
    edited_at: null,
    timestamp: row.created_at,
    favorite_id: null,
    forwarded_from: null,
    reactions: [],
    ...extra,
  } as BroadcastMessage
}

test('only photos and videos are viewer media; SVG stays a file (TG-103 rule)', () => {
  const base = fileRow(1).attachment
  expect(viewerKind({ ...base, mime_type: 'image/png' })).toBe('image')
  expect(viewerKind({ ...base, mime_type: 'VIDEO/MP4' })).toBe('video')
  expect(viewerKind({ ...base, mime_type: 'image/svg+xml' })).toBeNull()
  expect(viewerKind({ ...base, mime_type: 'application/pdf' })).toBeNull()
})

test('a timeline yields its live media, oldest first, with captions', () => {
  const messages: DisplayMessage[] = [
    broadcast(3, 'image/jpeg'),
    broadcast(1, 'video/mp4'),
    broadcast(2, 'application/pdf'),
    broadcast(4, 'image/png', { recalled_at: '2026-10-01T00:00:00Z' }),
    broadcast(5, 'image/png', { attachment: null }),
    { type: 'system', key: 's', content: 'joined' },
  ]
  const items = mediaFromMessages(messages)
  expect(items.map((item) => [item.attachmentId, item.kind, item.caption])).toEqual([
    ['a1', 'video', 'text 1'],
    ['a3', 'image', 'text 3'],
  ])
})

test('ordering parses timestamps (the server trims fractional seconds) and ties on id', () => {
  const a = media(1, { createdAt: '2026-10-01T00:00:00.5Z', messageId: 'm-b' })
  const b = media(2, { createdAt: '2026-10-01T00:00:00.123456Z', messageId: 'm-a' })
  expect(compareItems(b, a)).toBeLessThan(0)
  const tieA = media(3, { createdAt: '2026-10-01T00:00:00Z', messageId: 'm1' })
  const tieB = media(4, { createdAt: '2026-10-01T00:00:00Z', messageId: 'm2' })
  expect(compareItems(tieA, tieB)).toBeLessThan(0)
})

test('merge de-duplicates by attachment id and keeps the captioned copy', () => {
  const merged = mergeItems([media(2), media(1)], [media(2, { caption: null }), media(3, { caption: null })])
  expect(merged.map((item) => item.attachmentId)).toEqual(['a1', 'a2', 'a3'])
  expect(merged[1]?.caption).toBe('caption 2')
})

test('a /files page keeps its raw cursor and oldest row time even when it filters to nothing', () => {
  const page = toMediaPage({ items: [fileRow(9, 'application/zip'), fileRow(8, 'text/plain')], next_before: 'm8' })
  expect(page).toEqual({ items: [], nextBefore: 'm8', oldestAt: fileRow(8).created_at })
  expect(toMediaPage({ items: [], next_before: null })).toEqual({ items: [], nextBefore: null, oldestAt: null })
})

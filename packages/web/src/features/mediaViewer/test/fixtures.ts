/** Test builders for media items and `/files` pages. */
import type { ChatFileItem } from '@tg/core'
import type { MediaItem } from '../mediaItem'
import type { MediaPage } from '../mediaPager'

/** Minute `n` of a fixed day, so ordering is by `n`. */
export const at = (n: number) => new Date(Date.UTC(2026, 9, 1, 0, n)).toISOString()

export function media(n: number, overrides: Partial<MediaItem> = {}): MediaItem {
  return {
    attachmentId: `a${n}`,
    messageId: `m${n}`,
    kind: 'image',
    url: `/api/attachments/a${n}?key=k${n}`,
    fileName: `photo-${n}.jpg`,
    mimeType: 'image/jpeg',
    sizeBytes: 1000,
    isSensitive: false,
    sender: 'alice',
    senderId: 'u1',
    createdAt: at(n),
    caption: `caption ${n}`,
    ...overrides,
  }
}

export function fileRow(n: number, mime = 'image/jpeg'): ChatFileItem {
  return {
    message_id: `m${n}`,
    sender_id: 'u1',
    sender: 'alice',
    sender_avatar: '',
    created_at: at(n),
    attachment: {
      id: `a${n}`,
      file_name: `f-${n}`,
      mime_type: mime,
      size_bytes: 10,
      download_url: `/api/attachments/a${n}?key=k${n}`,
      is_sensitive: false,
    },
  }
}

/**
 * A fake `/files` backend over rows `1..count` (every `fileEvery`-th row a non-media file),
 * newest first, `pageSize` rows per page, cursor = message id. Records every `before`.
 */
export function fakeServer(count: number, pageSize: number, fileEvery = 0) {
  const rows = Array.from({ length: count }, (_, i) => i + 1)
    .reverse()
    .map((n) => (fileEvery && n % fileEvery === 0 ? fileRow(n, 'application/pdf') : fileRow(n)))
  const calls: Array<string | null> = []
  const fetchPage = async (before: string | null): Promise<MediaPage> => {
    calls.push(before)
    const start = before === null ? 0 : rows.findIndex((row) => row.message_id === before) + 1
    const slice = rows.slice(start, start + pageSize)
    const more = start + pageSize < rows.length
    const { toMediaPage } = await import('../chatMediaSource')
    return toMediaPage({ items: slice, next_before: more ? (slice.at(-1)?.message_id ?? null) : null })
  }
  return { fetchPage, calls }
}

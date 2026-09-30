import { describe, expect, test } from 'bun:test'
import type { BroadcastMessage, DisplayMessage, PendingBatch } from '@tg/core'
import { emptyPendingBatch, addPendingFiles } from '@tg/core'
import { albumItemsOf, createAlbumCollapser, rowMessageIds } from '../albumCollapse'
import { isAlbumBatch, sendPendingAlbum, type SendAlbumDeps } from '../sendPendingAlbum'

function message(id: string, extra: Partial<BroadcastMessage> = {}): BroadcastMessage {
  return {
    type: 'broadcast',
    message_id: id,
    sender_id: 'alice',
    sender: 'alice',
    sender_avatar: '',
    content: '',
    attachment: {
      id: `a-${id}`,
      file_name: `${id}.png`,
      mime_type: 'image/png',
      size_bytes: 1,
      download_url: '',
      is_sensitive: false,
    },
    reply_to: null,
    recalled_at: null,
    edited_at: null,
    timestamp: `2026-10-01T10:00:0${id.slice(-1)}Z`,
    favorite_id: null,
    forwarded_from: null,
    reactions: [],
    ...extra,
  }
}

describe('collapseAlbums', () => {
  test('a run of one album becomes one row carrying every item, keyed by the first', () => {
    const collapse = createAlbumCollapser()
    const before = message('m0', { attachment: null, content: 'hi' })
    const items = [1, 2, 3].map((n) => message(`m${n}`, { grouped_id: 'g1' }))
    const out = collapse([before, ...items])
    expect(out).toHaveLength(2)
    expect(out[0]).toBe(before)
    const row = out[1] as BroadcastMessage
    expect(row.message_id).toBe('m1')
    expect(row.timestamp).toBe(items[2]!.timestamp)
    expect(albumItemsOf(row)).toEqual(items)
    expect(rowMessageIds(row)).toEqual(['m1', 'm2', 'm3'])
    expect(rowMessageIds(before)).toEqual(['m0'])
  })

  test('the representative is reused while the items are unchanged (identity for the layout cache)', () => {
    const collapse = createAlbumCollapser()
    const items = [1, 2].map((n) => message(`m${n}`, { grouped_id: 'g1' }))
    const first = collapse(items)[0]
    const tail = message('m9', { attachment: null })
    const second = collapse([...items, tail])[0]
    expect(second).toBe(first)
    const grown = collapse([...items, message('m3', { grouped_id: 'g1' })])[0]
    expect(grown).not.toBe(first)
    expect(albumItemsOf(grown!)).toHaveLength(3)
  })

  test('a lone item, a recalled item, and another sender stay ordinary rows', () => {
    const collapse = createAlbumCollapser()
    const lone = [message('m1', { grouped_id: 'g1' }), message('m2', { attachment: null })]
    expect(collapse(lone)).toEqual(lone)
    const recalled = [
      message('m1', { grouped_id: 'g1' }),
      message('m2', { grouped_id: 'g1', recalled_at: '2026-10-01T11:00:00Z' }),
    ]
    expect(collapse(recalled)).toHaveLength(2)
    const mixed = [message('m1', { grouped_id: 'g1' }), message('m2', { grouped_id: 'g1', sender_id: 'bob' })]
    expect(collapse(mixed)).toHaveLength(2)
  })

  test('no albums: the input array itself comes back', () => {
    const collapse = createAlbumCollapser()
    const plain: DisplayMessage[] = [message('m1', { attachment: null })]
    expect(collapse(plain)).toBe(plain)
  })
})

interface FakeFile {
  name: string
  size: number
  type: string
  slice(start: number, end: number): Blob
}

function batchOf(types: string[], extra: Partial<PendingBatch<FakeFile>> = {}): PendingBatch<FakeFile> {
  const files = types.map((type, index) => ({
    file: { name: `f${index}`, size: 10, type, slice: () => new Blob() },
    name: `f${index}`,
    size: 10,
    mimeType: type,
    lastModified: index,
  }))
  let n = 0
  const { batch } = addPendingFiles(emptyPendingBatch<FakeFile>(), files, { createId: () => `p${n++}` })
  return { ...batch, caption: 'trip', ...extra }
}

describe('album sending', () => {
  test('only 2–10 photos/videos, not «as files», form an album', () => {
    expect(isAlbumBatch(batchOf(['image/png', 'video/mp4']))).toBe(true)
    expect(isAlbumBatch(batchOf(['image/png']))).toBe(false)
    expect(isAlbumBatch(batchOf(['image/png', 'application/pdf']))).toBe(false)
    expect(isAlbumBatch(batchOf(['image/png', 'image/png'], { sendAsFiles: true }))).toBe(false)
    // The composer's batch itself caps at 10 (MAX_PENDING_ATTACHMENTS), so 10 is the top case.
    expect(isAlbumBatch(batchOf(Array.from({ length: 10 }, () => 'image/png')))).toBe(true)
  })

  function harness(batch: PendingBatch<FakeFile>, fail: 'upload' | 'album' | null) {
    let current = batch
    const calls: string[] = []
    const deps: SendAlbumDeps<FakeFile> = {
      read: () => current,
      update: (change) => {
        current = change(current)
      },
      uploadChunks: async (file, onProgress) => {
        calls.push(`upload:${file.name}`)
        onProgress(10, 10)
        if (fail === 'upload' && file.name === 'f1') throw new Error('断网')
        return { uploadId: `u-${file.name}` }
      },
      createAlbum: async (ids, caption, reply) => {
        calls.push(`album:${ids.join(',')}:${caption}:${reply}`)
        if (fail === 'album') throw new Error('相册无效')
      },
      uploadStarted: () => {},
      uploadFinished: () => calls.push('finished'),
    }
    return { deps, calls, state: () => current }
  }

  test('every file uploads, then one album call carries every session, the caption and the reply', async () => {
    const h = harness(batchOf(['image/png', 'image/png', 'video/mp4']), null)
    const result = await sendPendingAlbum(h.deps, 'r1')
    expect(result).toEqual({ sent: 3, failed: 0 })
    expect(h.calls).toEqual(['upload:f0', 'upload:f1', 'upload:f2', 'album:u-f0,u-f1,u-f2:trip:r1', 'finished'])
    expect(h.state().items.every((item) => item.status === 'sent')).toBe(true)
  })

  test('an upload failure sends nothing and marks the whole album failed', async () => {
    const h = harness(batchOf(['image/png', 'image/png', 'image/png']), 'upload')
    const result = await sendPendingAlbum(h.deps, null)
    expect(result).toEqual({ sent: 0, failed: 3 })
    expect(h.calls.some((call) => call.startsWith('album:'))).toBe(false)
    expect(h.state().items.every((item) => item.status === 'failed')).toBe(true)
  })

  test('a refused album call leaves every item failed, never half sent', async () => {
    const h = harness(batchOf(['image/png', 'image/png']), 'album')
    const result = await sendPendingAlbum(h.deps, null)
    expect(result).toEqual({ sent: 0, failed: 2 })
    expect(h.state().items.map((item) => [item.status, item.error])).toEqual([
      ['failed', '相册无效'],
      ['failed', '相册无效'],
    ])
  })
})

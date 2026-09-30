// TG-104: the pending-attachment list — paste/drop of many files, limits, caption, progress.
import { describe, expect, test } from 'bun:test'
import type { PendingFileInput } from './pendingAttachments'
import {
  MAX_PENDING_ATTACHMENTS,
  addPendingFiles,
  classifyAttachment,
  emptyPendingBatch,
  pendingBatchTitle,
  removePendingFile,
  sendablePendingFiles,
  setPendingCaption,
  setSendAsFiles,
  updatePendingFile,
  uploadChatAction,
} from './pendingAttachments'

const image = (name: string, size = 100, lastModified = 1): PendingFileInput<string> => ({
  file: name,
  name,
  size,
  mimeType: 'image/png',
  lastModified,
})

const ids = () => {
  let next = 0
  return () => `p${++next}`
}

describe('pending attachments', () => {
  test('pasting several images queues them all as photos, in order', () => {
    const { batch, rejected } = addPendingFiles(emptyPendingBatch<string>(), [image('a.png'), image('b.png')], {
      createId: ids(),
    })
    expect(rejected).toEqual([])
    expect(batch.items.map((item) => [item.id, item.name, item.kind, item.status])).toEqual([
      ['p1', 'a.png', 'photo', 'ready'],
      ['p2', 'b.png', 'photo', 'ready'],
    ])
    expect(pendingBatchTitle(batch)).toBe('发送 2 张图片')
  })

  test('the same file twice is kept once; empty and oversized files are rejected with reasons', () => {
    const { batch, rejected } = addPendingFiles(
      emptyPendingBatch<string>(),
      [image('a.png'), image('a.png'), image('empty.png', 0), image('huge.png', 5000)],
      { createId: ids(), maxBytes: 1000 },
    )
    expect(batch.items).toHaveLength(1)
    expect(rejected).toEqual([
      { name: 'empty.png', reason: '文件为空' },
      { name: 'huge.png', reason: '文件超出大小限制' },
    ])
  })

  test('the album cap rejects the overflow instead of dropping it silently', () => {
    const inputs = Array.from({ length: MAX_PENDING_ATTACHMENTS + 2 }, (_, i) => image(`${i}.png`, 10, i))
    const { batch, rejected } = addPendingFiles(emptyPendingBatch<string>(), inputs, { createId: ids() })
    expect(batch.items).toHaveLength(MAX_PENDING_ATTACHMENTS)
    expect(rejected).toHaveLength(2)
  })

  test('classification, send-as-files, titles and chat actions', () => {
    expect(classifyAttachment('image/jpeg')).toBe('photo')
    expect(classifyAttachment('image/svg+xml')).toBe('file')
    expect(classifyAttachment('video/mp4')).toBe('video')
    expect(classifyAttachment('application/pdf')).toBe('file')
    const createId = ids()
    let batch = addPendingFiles(emptyPendingBatch<string>(), [image('a.png')], { createId }).batch
    batch = addPendingFiles(batch, [{ file: 'v', name: 'v.mp4', size: 5, mimeType: 'video/mp4' }], { createId }).batch
    expect(pendingBatchTitle(batch)).toBe('发送 2 个文件')
    const asFiles = setSendAsFiles(batch, true)
    expect(asFiles.items.map((item) => item.kind)).toEqual(['file', 'file'])
    expect(setSendAsFiles(asFiles, false).items.map((item) => item.kind)).toEqual(['photo', 'video'])
    expect(uploadChatAction('photo')).toBe('uploading_photo')
    expect(uploadChatAction('video')).toBe('uploading_video')
    expect(uploadChatAction('file')).toBe('uploading_document')
  })

  test('caption, progress, failure and removal', () => {
    let batch = addPendingFiles(emptyPendingBatch<string>(), [image('a.png'), image('b.png', 5)], {
      createId: ids(),
    }).batch
    batch = setPendingCaption(batch, '看这个')
    expect(setPendingCaption(batch, '看这个')).toBe(batch)
    batch = updatePendingFile(batch, 'p1', { status: 'sent', progress: 100 })
    batch = updatePendingFile(batch, 'p2', { status: 'failed', error: '网络错误' })
    expect(sendablePendingFiles(batch).map((item) => item.id)).toEqual(['p2'])
    expect(updatePendingFile(batch, 'missing', { progress: 1 })).toBe(batch)
    batch = removePendingFile(batch, 'p1')
    expect(batch.caption).toBe('看这个')
    batch = removePendingFile(batch, 'p2')
    expect(batch).toEqual(emptyPendingBatch())
  })
})

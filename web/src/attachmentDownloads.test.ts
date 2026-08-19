import { describe, expect, test } from 'bun:test'
import { partitionAttachmentBatches } from './attachmentDownloads'
import type { Attachment } from './types'

function attachment(id: number, size: number): Attachment {
  return {
    id: String(id),
    file_name: `${id}.bin`,
    mime_type: 'application/octet-stream',
    size_bytes: size,
    download_url: `/files/${id}`,
  }
}

describe('attachment download batches', () => {
  test('splits on file count and byte boundaries without dropping files', () => {
    const files = [attachment(1, 6), attachment(2, 6), attachment(3, 2), attachment(4, 20)]
    const batches = partitionAttachmentBatches(files, 2, 10)
    expect(batches.map((batch) => batch.map((file) => file.id))).toEqual([['1'], ['2', '3'], ['4']])
  })
})

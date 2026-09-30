/**
 * TG-104 uploads: the chunked REST client against a fake fetch (resume on 409, caption +
 * reply on complete) and the batch sender (caption once, per-item progress and failure,
 * chat actions around the whole batch).
 */
import { describe, expect, test } from 'bun:test'
import type { PendingBatch } from '@tg/core'
import { addPendingFiles, emptyPendingBatch, setPendingCaption } from '@tg/core'
import { sendPendingBatch } from '../sendPendingBatch'
import { UploadError, uploadAttachment } from '../uploadClient'

const file = (name: string, bytes: number) => new File([new Uint8Array(bytes)], name, { type: 'image/png' })

function fakeFetch(script: (url: string, init: RequestInit) => Response) {
  const calls: Array<{ url: string; method: string; body: unknown }> = []
  const fetchImpl = async (url: string, init: RequestInit = {}) => {
    calls.push({ url, method: init.method ?? 'GET', body: init.body })
    return script(url, init)
  }
  return { calls, fetchImpl }
}

const json = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })

describe('uploadAttachment', () => {
  test('create → chunks → complete, resuming at the server offset after a 409', async () => {
    let chunkCalls = 0
    const { calls, fetchImpl } = fakeFetch((url) => {
      if (url.endsWith('/attachments/uploads')) return json({ upload_id: 'up1', received_bytes: 0 })
      if (url.includes('/chunks')) {
        chunkCalls += 1
        if (chunkCalls === 2) return json({ received_bytes: 8 }, 409) // server already had more
        const offset = Number(new URL(url, 'http://x').searchParams.get('offset'))
        return json({ received_bytes: Math.min(10, offset + 4) })
      }
      if (url.endsWith('/complete')) return json({ id: 'msg-1' })
      return json({}, 500)
    })
    const progress: number[] = []
    const stored = await uploadAttachment(fetchImpl, {
      chatId: 'c 1',
      token: 'tok',
      file: file('a.png', 10),
      caption: '说明',
      replyTo: 'm9',
      onProgress: (done) => progress.push(done),
      chunkSize: 4,
    })
    expect(stored).toMatchObject({ id: 'msg-1' })
    expect(calls[0]!.url).toBe('/api/chats/c%201/attachments/uploads')
    expect(JSON.parse(calls[0]!.body as string)).toMatchObject({
      file_name: 'a.png',
      mime_type: 'image/png',
      size_bytes: 10,
    })
    expect(calls.filter((call) => call.method === 'PUT').map((call) => call.url)).toEqual([
      '/api/attachments/uploads/up1/chunks?offset=0',
      '/api/attachments/uploads/up1/chunks?offset=4',
      '/api/attachments/uploads/up1/chunks?offset=8',
    ])
    expect(JSON.parse(calls.at(-1)!.body as string)).toEqual({ content: '说明', reply_to: 'm9', is_sensitive: false })
    expect(progress.at(-1)).toBe(10)
  })

  test('maps a 413 on create to the size-limit copy', async () => {
    const { fetchImpl } = fakeFetch(() => json({}, 413))
    const attempt = uploadAttachment(fetchImpl, {
      chatId: 'c',
      token: 't',
      file: file('big.png', 3),
      caption: '',
      replyTo: null,
      onProgress: () => undefined,
    })
    await expect(attempt).rejects.toBeInstanceOf(UploadError)
    await expect(attempt).rejects.toThrow('文件超出大小限制')
  })
})

describe('sendPendingBatch', () => {
  test('caption and reply ride on the first file; failures stay for retry; actions bracket the batch', async () => {
    let ids = 0
    let batch: PendingBatch<File> = addPendingFiles(
      emptyPendingBatch<File>(),
      [file('a.png', 1), file('b.png', 2), file('c.png', 3)].map((f) => ({
        file: f,
        name: f.name,
        size: f.size,
        mimeType: f.type,
        lastModified: f.lastModified,
      })),
      { createId: () => `p${++ids}` },
    ).batch
    batch = setPendingCaption(batch, '三张图')
    const uploads: Array<{ name: string; caption: string; reply: string | null }> = []
    const events: string[] = []
    const deps = {
      read: () => batch,
      update: (change: (current: PendingBatch<File>) => PendingBatch<File>) => {
        batch = change(batch)
      },
      upload: async (f: File, caption: string, reply: string | null, onProgress: (d: number, t: number) => void) => {
        uploads.push({ name: f.name, caption, reply })
        onProgress(f.size, f.size)
        if (f.name === 'b.png' && uploads.length === 2) throw new Error('网络错误')
      },
      uploadStarted: (item: { kind: string }) => events.push(`start:${item.kind}`),
      uploadFinished: () => events.push('finish'),
    }

    expect(await sendPendingBatch(deps, 'm1')).toEqual({ sent: 2, failed: 1 })
    expect(uploads).toEqual([
      { name: 'a.png', caption: '三张图', reply: 'm1' },
      { name: 'b.png', caption: '', reply: null },
      { name: 'c.png', caption: '', reply: null },
    ])
    expect(batch.items.map((item) => [item.name, item.status, item.progress])).toEqual([
      ['a.png', 'sent', 100],
      ['b.png', 'failed', 100],
      ['c.png', 'sent', 100],
    ])
    expect(batch.items[1]!.error).toBe('网络错误')
    expect(events).toEqual(['start:photo', 'start:photo', 'start:photo', 'finish'])

    // Retry resends only the failed file, without repeating the caption.
    expect(await sendPendingBatch(deps, null)).toEqual({ sent: 1, failed: 0 })
    expect(uploads.at(-1)).toEqual({ name: 'b.png', caption: '', reply: null })
  })
})

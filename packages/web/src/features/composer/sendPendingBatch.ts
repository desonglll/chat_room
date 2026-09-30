/**
 * Sends a pending batch one file at a time: the caption and the reply target ride on the
 * first message, progress is reported per item, and the chat action (`uploading_photo`,
 * `uploading_document` …) is announced for the item in flight and cancelled at the end.
 * A failed item stays in the batch as `failed` so "重试" resends only what is left.
 * Framework-free so it is testable with a fake uploader.
 */
import type { PendingAttachment, PendingBatch } from '@tg/core'
import { sendablePendingFiles, updatePendingFile } from '@tg/core'
import type { UploadSource } from './uploadClient'

export interface SendBatchDeps<F extends UploadSource> {
  upload(
    file: F,
    caption: string,
    replyTo: string | null,
    onProgress: (done: number, total: number) => void,
  ): Promise<unknown>
  update(change: (batch: PendingBatch<F>) => PendingBatch<F>): void
  read(): PendingBatch<F>
  uploadStarted(item: PendingAttachment<F>): void
  uploadFinished(): void
}

export interface SendBatchResult {
  sent: number
  failed: number
}

export async function sendPendingBatch<F extends UploadSource>(
  deps: SendBatchDeps<F>,
  replyTo: string | null,
): Promise<SendBatchResult> {
  const queue = sendablePendingFiles(deps.read())
  // The caption goes out once: only if nothing of this batch has been sent yet.
  let caption = deps.read().items.some((item) => item.status === 'sent') ? '' : deps.read().caption.trim()
  let reply = replyTo
  let sent = 0
  let failed = 0
  try {
    for (const item of queue) {
      deps.update((batch) => updatePendingFile(batch, item.id, { status: 'uploading', progress: 0, error: '' }))
      deps.uploadStarted(item)
      try {
        await deps.upload(item.file, caption, reply, (done, total) => {
          const progress = total > 0 ? Math.round((done / total) * 100) : 100
          deps.update((batch) => updatePendingFile(batch, item.id, { progress }))
        })
        deps.update((batch) => updatePendingFile(batch, item.id, { status: 'sent', progress: 100 }))
        caption = ''
        reply = null
        sent += 1
      } catch (error) {
        const message = error instanceof Error && error.message ? error.message : '上传失败'
        deps.update((batch) => updatePendingFile(batch, item.id, { status: 'failed', error: message }))
        failed += 1
      }
    }
  } finally {
    deps.uploadFinished()
  }
  return { sent, failed }
}

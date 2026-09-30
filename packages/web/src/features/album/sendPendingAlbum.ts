/**
 * TG-403: send a pending batch of 2–10 photos/videos as ONE album. Every file's bytes go up
 * first (resumable chunked sessions, progress per item); then one `POST /api/chats/:id/albums`
 * turns every session into a message in a single transaction. Either the whole album appears
 * or none of it does: on failure every item is marked `failed` and «重试» re-sends the album
 * (the sessions resume by fingerprint, so bytes already uploaded are not sent again).
 *
 * Batches that are not an album — one item, a non-media file, or «以文件形式发送» — keep the
 * composer's one-message-per-file path (`sendPendingBatch`).
 */
import type { PendingAttachment, PendingBatch } from '@tg/core'
import { sendablePendingFiles, updatePendingFile } from '@tg/core'
import type { SendBatchResult } from '../composer/sendPendingBatch'
import type { UploadedSession, UploadSource } from '../composer/uploadClient'

export const MIN_ALBUM_ITEMS = 2
export const MAX_ALBUM_ITEMS = 10

/** Whether this batch goes out as an album (Telegram groups 2–10 photos/videos). */
export function isAlbumBatch<F>(batch: PendingBatch<F>): boolean {
  if (batch.sendAsFiles) return false
  if (batch.items.some((item) => item.status === 'sent')) return false
  const items = sendablePendingFiles(batch)
  return (
    items.length >= MIN_ALBUM_ITEMS &&
    items.length <= MAX_ALBUM_ITEMS &&
    items.every((item) => item.kind === 'photo' || item.kind === 'video')
  )
}

export interface SendAlbumDeps<F extends UploadSource> {
  read(): PendingBatch<F>
  update(change: (batch: PendingBatch<F>) => PendingBatch<F>): void
  uploadChunks(file: F, onProgress: (done: number, total: number) => void): Promise<UploadedSession>
  createAlbum(uploadIds: string[], caption: string, replyTo: string | null): Promise<unknown>
  uploadStarted(item: PendingAttachment<F>): void
  uploadFinished(): void
}

function errorText(error: unknown): string {
  return error instanceof Error && error.message ? error.message : '上传失败'
}

export async function sendPendingAlbum<F extends UploadSource>(
  deps: SendAlbumDeps<F>,
  replyTo: string | null,
): Promise<SendBatchResult> {
  const queue = sendablePendingFiles(deps.read())
  const caption = deps.read().caption.trim()
  const uploadIds: string[] = []
  const markAll = (change: Partial<Pick<PendingAttachment<F>, 'status' | 'progress' | 'error'>>) =>
    deps.update((batch) => queue.reduce((current, item) => updatePendingFile(current, item.id, change), batch))
  try {
    for (const item of queue) {
      deps.update((batch) => updatePendingFile(batch, item.id, { status: 'uploading', progress: 0, error: '' }))
      deps.uploadStarted(item)
      try {
        const session = await deps.uploadChunks(item.file, (done, total) => {
          // The last 1 % is the album commit, so a fully uploaded item never looks "sent".
          const progress = total > 0 ? Math.min(99, Math.round((done / total) * 100)) : 99
          deps.update((batch) => updatePendingFile(batch, item.id, { progress }))
        })
        uploadIds.push(session.uploadId)
      } catch (error) {
        markAll({ status: 'failed', error: errorText(error) })
        return { sent: 0, failed: queue.length }
      }
    }
    try {
      await deps.createAlbum(uploadIds, caption, replyTo)
    } catch (error) {
      markAll({ status: 'failed', error: errorText(error) })
      return { sent: 0, failed: queue.length }
    }
    markAll({ status: 'sent', progress: 100 })
    return { sent: queue.length, failed: 0 }
  } finally {
    deps.uploadFinished()
  }
}

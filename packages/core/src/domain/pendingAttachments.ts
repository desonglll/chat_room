import { t } from '../i18n/t'
/**
 * TG-104: the list of attachments waiting to be sent — pasted images, dropped files, the
 * attachment menu's picks — with one caption for the batch, like Telegram's send dialog.
 *
 * Generic over the host's file handle `F` (a browser `File`, an RN asset) so the list
 * logic stays platform-free. Every function returns a new list; nothing mutates.
 *
 * Batch rules:
 * - at most `MAX_PENDING_ATTACHMENTS` (10, Telegram's album size); extra picks are
 *   rejected with a reason rather than silently dropped;
 * - empty files and files over the server's `max_upload_bytes` are rejected up front;
 * - the same file (name + size + lastModified) picked twice is kept once;
 * - the caption rides on the FIRST sent item (the server stores one text per message).
 */

export type AttachmentKind = 'photo' | 'video' | 'file'

export type PendingStatus = 'ready' | 'uploading' | 'failed' | 'sent'

export interface PendingFileInput<F> {
  file: F
  name: string
  size: number
  mimeType: string
  /** Host timestamp, only used to recognise the same file picked twice. */
  lastModified?: number
}

export interface PendingAttachment<F> {
  id: string
  file: F
  name: string
  size: number
  mimeType: string
  kind: AttachmentKind
  status: PendingStatus
  /** 0–100 while uploading. */
  progress: number
  error: string
  fingerprint: string
}

export interface PendingBatch<F> {
  items: readonly PendingAttachment<F>[]
  caption: string
  /** «以文件形式发送»: skip photo/video treatment (Telegram's "send without compression"). */
  sendAsFiles: boolean
}

export interface RejectedFile {
  name: string
  reason: string
}

export const MAX_PENDING_ATTACHMENTS = 10

export const EMPTY_PENDING_BATCH: PendingBatch<never> = Object.freeze({
  items: Object.freeze([]) as readonly never[],
  caption: '',
  sendAsFiles: false,
})

export function emptyPendingBatch<F>(): PendingBatch<F> {
  return EMPTY_PENDING_BATCH as PendingBatch<F>
}

export function classifyAttachment(mimeType: string, sendAsFiles = false): AttachmentKind {
  if (sendAsFiles) return 'file'
  if (/^image\/(png|jpe?g|gif|webp|avif|bmp)$/i.test(mimeType)) return 'photo'
  if (/^video\//i.test(mimeType)) return 'video'
  return 'file'
}

export function addPendingFiles<F>(
  batch: PendingBatch<F>,
  inputs: readonly PendingFileInput<F>[],
  options: { createId: () => string; maxBytes?: number },
): { batch: PendingBatch<F>; rejected: RejectedFile[] } {
  const items = [...batch.items]
  const rejected: RejectedFile[] = []
  for (const input of inputs) {
    const fingerprint = `${input.name}:${input.size}:${input.lastModified ?? 0}`
    if (items.some((item) => item.fingerprint === fingerprint)) continue
    if (input.size <= 0) {
      rejected.push({ name: input.name, reason: t('c.domain.dfdb42') })
      continue
    }
    if (options.maxBytes !== undefined && input.size > options.maxBytes) {
      rejected.push({ name: input.name, reason: t('c.domain.f9c82d') })
      continue
    }
    if (items.length >= MAX_PENDING_ATTACHMENTS) {
      rejected.push({ name: input.name, reason: t('c.domain.decc4d', MAX_PENDING_ATTACHMENTS) })
      continue
    }
    const mimeType = input.mimeType || 'application/octet-stream'
    items.push({
      id: options.createId(),
      file: input.file,
      name: input.name,
      size: input.size,
      mimeType,
      kind: classifyAttachment(mimeType, batch.sendAsFiles),
      status: 'ready',
      progress: 0,
      error: '',
      fingerprint,
    })
  }
  return { batch: { ...batch, items }, rejected }
}

export function removePendingFile<F>(batch: PendingBatch<F>, id: string): PendingBatch<F> {
  if (!batch.items.some((item) => item.id === id)) return batch
  const items = batch.items.filter((item) => item.id !== id)
  // Closing the last preview closes the dialog: the caption goes with it.
  return items.length === 0 ? emptyPendingBatch() : { ...batch, items }
}

export function setPendingCaption<F>(batch: PendingBatch<F>, caption: string): PendingBatch<F> {
  return batch.caption === caption ? batch : { ...batch, caption }
}

export function setSendAsFiles<F>(batch: PendingBatch<F>, sendAsFiles: boolean): PendingBatch<F> {
  if (batch.sendAsFiles === sendAsFiles) return batch
  return {
    ...batch,
    sendAsFiles,
    items: batch.items.map((item) => ({ ...item, kind: classifyAttachment(item.mimeType, sendAsFiles) })),
  }
}

export function updatePendingFile<F>(
  batch: PendingBatch<F>,
  id: string,
  change: Partial<Pick<PendingAttachment<F>, 'status' | 'progress' | 'error'>>,
): PendingBatch<F> {
  if (!batch.items.some((item) => item.id === id)) return batch
  return { ...batch, items: batch.items.map((item) => (item.id === id ? { ...item, ...change } : item)) }
}

/** What still has to go out: ready or previously failed items, in order. */
export function sendablePendingFiles<F>(batch: PendingBatch<F>): PendingAttachment<F>[] {
  return batch.items.filter((item) => item.status === 'ready' || item.status === 'failed')
}

/** The send dialog's title, Telegram-style: «发送 3 张图片» / «发送 2 个视频» / «发送 4 个文件». */
export function pendingBatchTitle<F>(batch: PendingBatch<F>): string {
  const count = batch.items.length
  if (count === 0) return ''
  if (batch.items.every((item) => item.kind === 'photo')) return t('c.domain.c502d3', count)
  if (batch.items.every((item) => item.kind === 'video')) return t('c.domain.ffd6d7', count)
  return t('c.domain.51ccc1', count)
}

/** The typing action to announce while this item uploads (TG-107 `createChatActionSender`). */
export function uploadChatAction(kind: AttachmentKind): 'uploading_photo' | 'uploading_video' | 'uploading_document' {
  if (kind === 'photo') return 'uploading_photo'
  if (kind === 'video') return 'uploading_video'
  return 'uploading_document'
}

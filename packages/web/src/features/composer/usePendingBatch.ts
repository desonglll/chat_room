/**
 * React state around the pending-attachment list (`@tg/core` `pendingAttachments`):
 * adding pasted / dropped / picked files, object-URL previews (revoked when an item
 * leaves or the chat changes), caption, and sending through `sendPendingBatch`.
 */
import { useCallback, useEffect, useRef, useState } from 'react'
import type { PendingAttachment, PendingBatch, RejectedFile } from '@tg/core'
import {
  addPendingFiles,
  authStore,
  createRandomUuid,
  emptyPendingBatch,
  removePendingFile,
  selectToken,
  setPendingCaption,
  setSendAsFiles,
} from '@tg/core'
import { browserFetch } from '../../app/platform'
import { activeTopicId } from '../forum/activeTopic'
import type { ComposerController } from './composerController'
import { isAlbumBatch, sendPendingAlbum } from '../album/sendPendingAlbum'
import { sendAlbum } from '../album/albumApi'
import { sendPendingBatch } from './sendPendingBatch'
import { uploadAttachment, uploadChunks } from './uploadClient'

export interface PendingBatchHandle {
  batch: PendingBatch<File>
  rejected: RejectedFile[]
  sending: boolean
  add(files: readonly File[]): void
  remove(id: string): void
  setCaption(caption: string): void
  setSendAsFiles(value: boolean): void
  previewUrl(item: PendingAttachment<File>): string
  send(): Promise<void>
  clear(): void
}

export function usePendingBatch(chatId: string, controller: ComposerController, replyTo: string | null) {
  const [batch, setBatch] = useState<PendingBatch<File>>(emptyPendingBatch)
  const [rejected, setRejected] = useState<RejectedFile[]>([])
  const [sending, setSending] = useState(false)
  const batchRef = useRef(batch)
  const previews = useRef(new Map<string, string>())

  const update = useCallback((change: (current: PendingBatch<File>) => PendingBatch<File>) => {
    batchRef.current = change(batchRef.current)
    setBatch(batchRef.current)
    // Revoke previews of items that left the batch.
    for (const [id, url] of previews.current) {
      if (!batchRef.current.items.some((item) => item.id === id)) {
        URL.revokeObjectURL(url)
        previews.current.delete(id)
      }
    }
  }, [])

  const clear = useCallback(() => {
    update(() => emptyPendingBatch())
    setRejected([])
  }, [update])

  // A chat switch drops the unsent batch (its object URLs with it).
  useEffect(() => clear, [chatId, clear])

  const add = useCallback(
    (files: readonly File[]) => {
      if (files.length === 0) return
      let refused: RejectedFile[] = []
      update((current) => {
        const result = addPendingFiles(
          current,
          files.map((file) => ({
            file,
            name: file.name || 'image.png',
            size: file.size,
            mimeType: file.type,
            lastModified: file.lastModified,
          })),
          { createId: createRandomUuid },
        )
        refused = result.rejected
        return result.batch
      })
      setRejected(refused)
    },
    [update],
  )

  const send = useCallback(async () => {
    const token = selectToken(authStore.getState())
    if (!token || batchRef.current.items.length === 0) return
    setSending(true)
    try {
      const topicId = activeTopicId(chatId)
      const result = isAlbumBatch(batchRef.current)
        ? await sendPendingAlbum<File>(
            {
              read: () => batchRef.current,
              update,
              uploadChunks: (file, onProgress) => uploadChunks(browserFetch, { chatId, token, file, onProgress }),
              createAlbum: (uploadIds, caption, reply) =>
                sendAlbum(browserFetch, { chatId, token, uploadIds, caption, replyTo: reply, topicId }),
              uploadStarted: (item) => controller.uploadStarted(item.kind),
              uploadFinished: () => controller.uploadFinished(),
            },
            replyTo,
          )
        : await sendPendingBatch<File>(
            {
              read: () => batchRef.current,
              update,
              upload: (file, caption, reply, onProgress) =>
                uploadAttachment(browserFetch, {
                  chatId,
                  token,
                  file,
                  caption,
                  replyTo: reply,
                  topicId,
                  onProgress,
                }),
              uploadStarted: (item) => controller.uploadStarted(item.kind),
              uploadFinished: () => controller.uploadFinished(),
            },
            replyTo,
          )
      if (result.sent > 0) controller.consumeReply()
      if (result.failed === 0) clear()
    } finally {
      setSending(false)
    }
  }, [chatId, clear, controller, replyTo, update])

  const previewUrl = useCallback((item: PendingAttachment<File>) => {
    let url = previews.current.get(item.id)
    if (!url) {
      url = URL.createObjectURL(item.file)
      previews.current.set(item.id, url)
    }
    return url
  }, [])

  return {
    batch,
    rejected,
    sending,
    add,
    remove: (id: string) => update((current) => removePendingFile(current, id)),
    setCaption: (caption: string) => update((current) => setPendingCaption(current, caption)),
    setSendAsFiles: (value: boolean) => update((current) => setSendAsFiles(current, value)),
    previewUrl,
    send,
    clear,
  } satisfies PendingBatchHandle
}

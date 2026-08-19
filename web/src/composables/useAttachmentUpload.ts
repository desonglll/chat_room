import { ref, watch, type Ref } from 'vue'
import { cancelUploadSession, formatUploadLimit, listUploadSessions } from '../api'
import { loadUploadDraft, removeUploadDraft, saveUploadDraft, type UploadDraft } from '../uploadDraftStorage'
import { uploadFileInChunks, type ChunkedUploadProgress } from './useChunkedUpload'
import type { AttachmentUploadSession, BroadcastMessage, Room } from '../types'

interface UploadOptions {
  room: Ref<Room | null>
  token: Ref<string>
  password: Ref<string>
  authenticated: () => boolean
  maxBytes: Ref<number>
  append: (message: BroadcastMessage) => void
  showError: (message: string) => void
}

export function useAttachmentUpload(options: UploadOptions) {
  const uploading = ref(false)
  const progress = ref<ChunkedUploadProgress | null>(null)
  const pendingUploads = ref<AttachmentUploadSession[]>([])

  async function refreshPending(): Promise<void> {
    const roomId = options.room.value?.id
    const token = options.token.value
    if (!roomId || !token) {
      pendingUploads.value = []
      return
    }
    try {
      const sessions = await listUploadSessions(roomId, token)
      if (options.room.value?.id === roomId && options.token.value === token) pendingUploads.value = sessions
    } catch {
      if (options.room.value?.id === roomId) pendingUploads.value = []
    }
  }

  async function runUpload(file: File, draft: UploadDraft, preferredFingerprint?: string): Promise<void> {
    const room = options.room.value
    if (!room) return
    if (file.size > options.maxBytes.value) {
      throw new Error(`单个文件不能超过 ${formatUploadLimit(options.maxBytes.value)}`)
    }
    const result = await uploadFileInChunks({
      roomId: room.id,
      token: options.token.value,
      password: options.password.value,
      file,
      ...draft,
      preferredFingerprint,
      onProgress: (next) => {
        progress.value = next
      },
      onSession: async (uploadId) => {
        saveUploadDraft(window.localStorage, uploadId, draft)
        await refreshPending()
      },
    })
    removeUploadDraft(window.localStorage, result.uploadId)
    if (options.room.value?.id === room.id) options.append(result.message)
  }

  async function withUploadLock(action: () => Promise<void>): Promise<void> {
    if (!options.token.value || !options.authenticated() || uploading.value) return
    uploading.value = true
    progress.value = null
    try {
      await action()
    } catch (caught) {
      options.showError(caught instanceof Error ? caught.message : '文件上传失败')
    } finally {
      uploading.value = false
      progress.value = null
      await refreshPending()
    }
  }

  async function upload(files: File[], content = '', replyTo = '', isSensitive = false): Promise<void> {
    await withUploadLock(async () => {
      for (const [index, file] of files.entries()) {
        await runUpload(file, {
          content: index === 0 ? content : '',
          replyTo: index === 0 ? replyTo : '',
          isSensitive,
        })
      }
    })
  }

  async function resume(session: AttachmentUploadSession, file: File): Promise<void> {
    if (file.name !== session.file_name || file.size !== session.declared_size_bytes) {
      options.showError('请选择名称和大小均相同的原文件')
      return
    }
    const draft = loadUploadDraft(window.localStorage, session.id)
    await withUploadLock(() => runUpload(file, draft, session.fingerprint))
  }

  async function cancel(session: AttachmentUploadSession): Promise<void> {
    if (!options.token.value || uploading.value) return
    try {
      await cancelUploadSession(session.id, options.token.value)
      removeUploadDraft(window.localStorage, session.id)
      await refreshPending()
    } catch (caught) {
      options.showError(caught instanceof Error ? caught.message : '取消上传失败')
    }
  }

  watch(
    [options.room, options.token],
    () => {
      void refreshPending()
    },
    { immediate: true },
  )

  return { upload, resume, cancel, refreshPending, uploading, progress, pendingUploads }
}

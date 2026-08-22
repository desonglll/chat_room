import { onBeforeUnmount, ref, watch, type Ref } from 'vue'
import { formatUploadLimit } from '../api'
import { cancelUploadSession, listUploadSessions } from '../attachmentUploadApi'
import { loadUploadDraft, removeUploadDraft, saveUploadDraft, type UploadDraft } from '../uploadDraftStorage'
import { createRandomUuid } from '../randomUuid'
import { uploadFileInChunks } from './useChunkedUpload'
import type { AttachmentUploadSession, BroadcastMessage, Room, UploadMessage } from '../types'

const MAX_PARALLEL_UPLOADS = 3

interface UploadOptions {
  room: Ref<Room | null>
  token: Ref<string>
  password: Ref<string>
  authenticated: () => boolean
  maxBytes: Ref<number>
  append: (message: UploadMessage) => void
  update: (key: string, patch: Partial<UploadMessage>) => void
  complete: (key: string, message: BroadcastMessage) => void
  remove: (key: string) => void
  showError: (message: string) => void
}

interface UploadSource {
  key: string
  roomId: string
  token: string
  password: string
  file: File
  draft: UploadDraft
  preferredFingerprint?: string
  previewUrl: string
  uploadId?: string
  controller?: AbortController
  cancelled?: boolean
}

export function useAttachmentUpload(options: UploadOptions) {
  const pendingUploads = ref<AttachmentUploadSession[]>([])
  const runningCount = ref(0)
  const sources = new Map<string, UploadSource>()
  const queue: string[] = []

  async function refreshPending(): Promise<void> {
    const roomId = options.room.value?.id
    const token = options.token.value
    if (!roomId || !token) {
      pendingUploads.value = []
      return
    }
    try {
      const activeIds = new Set(Array.from(sources.values(), (source) => source.uploadId).filter(Boolean))
      const sessions = await listUploadSessions(roomId, token)
      if (options.room.value?.id === roomId && options.token.value === token) {
        pendingUploads.value = sessions.filter((session) => !activeIds.has(session.id))
      }
    } catch {
      if (options.room.value?.id === roomId) pendingUploads.value = []
    }
  }

  function createSource(file: File, draft: UploadDraft, preferredFingerprint?: string): UploadSource {
    const room = options.room.value as Room
    const key = `upload-${createRandomUuid()}`
    const previewUrl = file.type.startsWith('image/') || file.type.startsWith('video/') ? URL.createObjectURL(file) : ''
    return {
      key,
      roomId: room.id,
      token: options.token.value,
      password: options.password.value,
      file,
      draft,
      preferredFingerprint,
      previewUrl,
    }
  }

  function appendSource(source: UploadSource): void {
    sources.set(source.key, source)
    options.append({
      type: 'upload',
      key: source.key,
      room_id: source.roomId,
      file_name: source.file.name,
      mime_type: source.file.type || 'application/octet-stream',
      size_bytes: source.file.size,
      preview_url: source.previewUrl,
      is_sensitive: source.draft.isSensitive,
      content: source.draft.content,
      phase: 'queued',
      processed_bytes: 0,
      total_bytes: source.file.size,
      status: 'pending',
      error: '',
      timestamp: new Date().toISOString(),
    })
    queue.push(source.key)
  }

  function releaseSource(source: UploadSource): void {
    if (source.previewUrl) URL.revokeObjectURL(source.previewUrl)
    sources.delete(source.key)
  }

  function pumpQueue(): void {
    while (runningCount.value < MAX_PARALLEL_UPLOADS && queue.length) {
      const key = queue.shift() as string
      const source = sources.get(key)
      if (source && !source.controller && !source.cancelled) void runUpload(source)
    }
  }

  async function runUpload(source: UploadSource): Promise<void> {
    source.controller = new AbortController()
    runningCount.value += 1
    options.update(source.key, { status: 'pending', error: '' })
    try {
      const result = await uploadFileInChunks({
        roomId: source.roomId,
        token: source.token,
        password: source.password,
        file: source.file,
        ...source.draft,
        preferredFingerprint: source.preferredFingerprint,
        signal: source.controller.signal,
        onProgress: (progress) => {
          options.update(source.key, {
            phase: progress.phase,
            processed_bytes: progress.processedBytes,
            total_bytes: progress.totalBytes,
          })
        },
        onSession: (uploadId) => {
          source.uploadId = uploadId
          saveUploadDraft(window.localStorage, uploadId, source.draft)
        },
      })
      removeUploadDraft(window.localStorage, result.uploadId)
      if (options.room.value?.id === source.roomId) options.complete(source.key, result.message)
      releaseSource(source)
    } catch (caught) {
      if (!source.cancelled) {
        const message = caught instanceof Error ? caught.message : '文件上传失败'
        if (options.room.value?.id === source.roomId) {
          options.update(source.key, { status: 'failed', error: message })
          options.showError(message)
        } else {
          releaseSource(source)
        }
      }
    } finally {
      source.controller = undefined
      runningCount.value = Math.max(0, runningCount.value - 1)
      void refreshPending()
      pumpQueue()
    }
  }

  function upload(files: File[], content = '', replyTo = '', isSensitive = false): void {
    if (!options.room.value || !options.token.value || !options.authenticated()) return
    for (const [index, file] of files.entries()) {
      if (!file.size || file.size > options.maxBytes.value) {
        options.showError(`${file.name} 不能超过 ${formatUploadLimit(options.maxBytes.value)}，且不能为空`)
        continue
      }
      appendSource(
        createSource(file, {
          content: index === 0 ? content : '',
          replyTo: index === 0 ? replyTo : '',
          isSensitive,
        }),
      )
    }
    pumpQueue()
  }

  function resume(session: AttachmentUploadSession, file: File): void {
    if (!options.room.value || !options.token.value || !options.authenticated()) return
    if (file.name !== session.file_name || file.size !== session.declared_size_bytes) {
      options.showError('请选择名称和大小均相同的原文件')
      return
    }
    const source = createSource(file, loadUploadDraft(window.localStorage, session.id), session.fingerprint)
    source.uploadId = session.id
    pendingUploads.value = pendingUploads.value.filter((item) => item.id !== session.id)
    appendSource(source)
    pumpQueue()
  }

  function retry(key: string): void {
    const source = sources.get(key)
    if (!source || source.controller || queue.includes(key)) return
    source.cancelled = false
    options.update(key, { phase: 'queued', status: 'pending', error: '', processed_bytes: 0 })
    queue.push(key)
    pumpQueue()
  }

  async function cancelTask(key: string): Promise<void> {
    const source = sources.get(key)
    if (!source) return
    source.cancelled = true
    source.controller?.abort()
    const queuedIndex = queue.indexOf(key)
    if (queuedIndex >= 0) queue.splice(queuedIndex, 1)
    options.remove(key)
    releaseSource(source)
    if (source.uploadId) {
      removeUploadDraft(window.localStorage, source.uploadId)
      await cancelUploadSession(source.uploadId, source.token).catch(() => {})
    }
    void refreshPending()
  }

  async function cancel(session: AttachmentUploadSession): Promise<void> {
    if (!options.token.value) return
    try {
      await cancelUploadSession(session.id, options.token.value)
      removeUploadDraft(window.localStorage, session.id)
      await refreshPending()
    } catch (caught) {
      options.showError(caught instanceof Error ? caught.message : '取消上传失败')
    }
  }

  watch([options.room, options.token], () => void refreshPending(), { immediate: true })
  onBeforeUnmount(() => {
    for (const source of sources.values()) {
      source.cancelled = true
      source.controller?.abort()
      if (source.previewUrl) URL.revokeObjectURL(source.previewUrl)
    }
    sources.clear()
  })

  return { upload, resume, retry, cancelTask, cancel, refreshPending, pendingUploads }
}

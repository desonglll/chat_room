import { ref, type Ref } from 'vue'
import { uploadAttachment } from '../api'
import { CHUNKED_UPLOAD_THRESHOLD, uploadFileInChunks, type ChunkedUploadProgress } from './useChunkedUpload'
import type { BroadcastMessage, Room } from '../types'

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

  async function upload(files: File[], content = '', replyTo = '', isSensitive = false): Promise<void> {
    const room = options.room.value
    if (!room || !options.token.value || !options.authenticated() || uploading.value) return
    uploading.value = true
    progress.value = null
    try {
      for (const [index, file] of files.entries()) {
        const fileContent = index === 0 ? content : ''
        const fileReplyTo = index === 0 ? replyTo : ''
        const message = file.size > CHUNKED_UPLOAD_THRESHOLD
          ? await uploadFileInChunks(
              room.id,
              options.token.value,
              options.password.value,
              file,
              fileContent,
              fileReplyTo,
              isSensitive,
              (next) => { progress.value = next },
            )
          : await uploadAttachment(
              room.id,
              file,
              options.token.value,
              options.password.value,
              fileContent,
              fileReplyTo,
              options.maxBytes.value,
              isSensitive,
            )
        if (options.room.value?.id === room.id) options.append(message)
      }
    } catch (caught) {
      options.showError(caught instanceof Error ? caught.message : '文件上传失败')
    } finally {
      uploading.value = false
      progress.value = null
    }
  }

  return { upload, uploading, progress }
}

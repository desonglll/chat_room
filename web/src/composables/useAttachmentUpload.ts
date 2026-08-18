import { ref, type Ref } from 'vue'
import { uploadAttachment } from '../api'
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

  async function upload(files: File[], content = '', replyTo = ''): Promise<void> {
    const room = options.room.value
    if (!room || !options.token.value || !options.authenticated() || uploading.value) return
    uploading.value = true
    try {
      for (const [index, file] of files.entries()) {
        const message = await uploadAttachment(
          room.id,
          file,
          options.token.value,
          options.password.value,
          index === 0 ? content : '',
          index === 0 ? replyTo : '',
          options.maxBytes.value,
        )
        if (options.room.value?.id === room.id) options.append(message)
      }
    } catch (caught) {
      options.showError(caught instanceof Error ? caught.message : '文件上传失败')
    } finally {
      uploading.value = false
    }
  }

  return { upload, uploading }
}

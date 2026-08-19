import { ref } from 'vue'
import { useToast } from 'primevue/usetoast'
import { downloadAttachmentArchives, type DownloadProgress } from '../attachmentDownloads'
import type { Attachment } from '../types'

export function useAttachmentDownloads(roomName: () => string) {
  const downloading = ref(false)
  const downloadProgress = ref<DownloadProgress | null>(null)
  const toast = useToast()
  let controller: AbortController | null = null

  async function download(attachments: Attachment[]): Promise<void> {
    if (!attachments.length || downloading.value) return
    const nextController = new AbortController()
    controller = nextController
    downloading.value = true
    downloadProgress.value = {
      stage: 'downloading',
      completedFiles: 0,
      totalFiles: attachments.length,
      receivedBytes: 0,
      totalBytes: attachments.reduce((sum, file) => sum + file.size_bytes, 0),
      batchIndex: 1,
      batchCount: 1,
      percent: 0,
    }
    try {
      await downloadAttachmentArchives(attachments, roomName(), {
        signal: nextController.signal,
        onProgress: (progress) => {
          downloadProgress.value = progress
        },
      })
      toast.add({ severity: 'success', summary: `已保存 ${attachments.length} 个文件`, life: 2600 })
    } catch (caught) {
      const cancelled = caught instanceof DOMException && caught.name === 'AbortError'
      toast.add({
        severity: cancelled ? 'secondary' : 'error',
        summary: cancelled ? '已取消批量保存' : caught instanceof Error ? caught.message : '批量保存失败',
        life: 3200,
      })
    } finally {
      if (controller === nextController) {
        controller = null
        downloading.value = false
        downloadProgress.value = null
      }
    }
  }

  function cancel(): void {
    controller?.abort()
  }

  return { cancel, download, downloading, downloadProgress }
}

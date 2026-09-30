/**
 * Upload progress and download cache state. The percent math is the migrated
 * `domain/attachmentUploadProgress`; the chunked-session orchestration itself is the M1
 * media task's, which will drive these actions.
 */
import { createStore } from 'zustand/vanilla'
import { uploadPercent } from '../domain/attachmentUploadProgress'
import type { UploadPhase, UploadTaskStatus } from '../domain/messageView'

export interface UploadEntry {
  uploadId: string
  chatId: string
  fileName: string
  phase: UploadPhase
  processedBytes: number
  totalBytes: number
  status: UploadTaskStatus
  error: string
}

export interface MediaState {
  uploads: Record<string, UploadEntry>
  /** attachment id → locally cached object handle/URL, host-defined. */
  downloads: Record<string, string>
  beginUpload(entry: Omit<UploadEntry, 'status' | 'error'>): void
  reportUploadProgress(uploadId: string, phase: UploadPhase, processedBytes: number): void
  failUpload(uploadId: string, error: string): void
  finishUpload(uploadId: string): void
  cacheDownload(attachmentId: string, handle: string): void
  evictDownload(attachmentId: string): void
}

export const createMediaStore = () =>
  createStore<MediaState>()((set) => ({
    uploads: {},
    downloads: {},
    beginUpload: (entry) =>
      set((state) => ({
        uploads: { ...state.uploads, [entry.uploadId]: { ...entry, status: 'pending', error: '' } },
      })),
    reportUploadProgress: (uploadId, phase, processedBytes) =>
      set((state) => {
        const upload = state.uploads[uploadId]
        if (!upload) return {}
        return { uploads: { ...state.uploads, [uploadId]: { ...upload, phase, processedBytes } } }
      }),
    failUpload: (uploadId, error) =>
      set((state) => {
        const upload = state.uploads[uploadId]
        if (!upload) return {}
        return { uploads: { ...state.uploads, [uploadId]: { ...upload, status: 'failed', error } } }
      }),
    finishUpload: (uploadId) =>
      set((state) => {
        const { [uploadId]: _removed, ...rest } = state.uploads
        return { uploads: rest }
      }),
    cacheDownload: (attachmentId, handle) =>
      set((state) => ({ downloads: { ...state.downloads, [attachmentId]: handle } })),
    evictDownload: (attachmentId) =>
      set((state) => {
        const { [attachmentId]: _removed, ...rest } = state.downloads
        return { downloads: rest }
      }),
  }))

export type MediaStore = ReturnType<typeof createMediaStore>

export const selectUploadPercent = (uploadId: string) => (state: MediaState) => {
  const upload = state.uploads[uploadId]
  return upload ? uploadPercent(upload.phase, upload.processedBytes, upload.totalBytes) : 0
}

export const mediaStore = createMediaStore()

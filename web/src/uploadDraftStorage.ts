import { storageGet, storageSet } from './browserStorage'

export interface UploadDraft {
  content: string
  replyTo: string
  isSensitive: boolean
}

const keyOf = (uploadId: string) => `chat-room.upload-draft.${uploadId}`

export function saveUploadDraft(storage: Storage, uploadId: string, draft: UploadDraft): void {
  storageSet(storage, keyOf(uploadId), JSON.stringify(draft))
}

export function loadUploadDraft(storage: Storage, uploadId: string): UploadDraft {
  const encoded = storageGet(storage, keyOf(uploadId))
  if (!encoded) return { content: '', replyTo: '', isSensitive: false }
  try {
    const parsed = JSON.parse(encoded) as Partial<UploadDraft>
    return {
      content: typeof parsed.content === 'string' ? parsed.content : '',
      replyTo: typeof parsed.replyTo === 'string' ? parsed.replyTo : '',
      isSensitive: parsed.isSensitive === true,
    }
  } catch {
    return { content: '', replyTo: '', isSensitive: false }
  }
}

export function removeUploadDraft(storage: Storage, uploadId: string): void {
  storageSet(storage, keyOf(uploadId), '')
}

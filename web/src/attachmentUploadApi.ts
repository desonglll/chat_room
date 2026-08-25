import { authHeaders, request, storedMessageToBroadcast } from './api'
import type { AttachmentUploadSession, BroadcastMessage, StoredMessage } from './types'

export interface CreateUploadSessionResult {
  upload_id: string
  received_bytes: number
  declared_size_bytes: number
  deduplicated: boolean
  direct_upload?: DirectUploadTarget
}

export interface DirectUploadTarget {
  method: string
  url: string
  headers: Record<string, string>
  expires_at: string
}

export async function createUploadSession(
  roomId: string,
  token: string,
  password: string,
  file: File,
  fingerprint: string,
  contentHash: string,
  signal?: AbortSignal,
): Promise<CreateUploadSessionResult> {
  const headers: Record<string, string> = { ...authHeaders(token), 'Content-Type': 'application/json' }
  if (password) headers['x-room-password'] = password
  const response = await request(`/api/rooms/${encodeURIComponent(roomId)}/attachments/uploads`, {
    method: 'POST',
    headers,
    signal,
    body: JSON.stringify({
      file_name: file.name,
      mime_type: file.type || 'application/octet-stream',
      size_bytes: file.size,
      fingerprint,
      content_hash: contentHash,
    }),
  })
  if (response.status === 413) throw new Error('文件超出大小限制')
  if (response.status === 409) throw new Error('所选文件与未完成上传的内容不一致')
  if (!response.ok) throw new Error(`创建上传会话失败：${response.status}`)
  return response.json() as Promise<CreateUploadSessionResult>
}

export async function uploadChunk(
  uploadId: string,
  token: string,
  offset: number,
  chunk: Blob,
  signal?: AbortSignal,
): Promise<number> {
  const response = await request(`/api/attachments/uploads/${encodeURIComponent(uploadId)}/chunks?offset=${offset}`, {
    method: 'PUT',
    headers: { ...authHeaders(token), 'Content-Type': 'application/octet-stream' },
    body: chunk,
    signal,
  })
  if (response.status === 409) {
    const body = (await response.json()) as { received_bytes: number }
    const error = new Error('分片偏移量不匹配') as Error & { receivedBytes: number }
    error.receivedBytes = body.received_bytes
    throw error
  }
  if (!response.ok) throw new Error(`上传分片失败：${response.status}`)
  const body = (await response.json()) as { received_bytes: number }
  return body.received_bytes
}

export function uploadDirect(
  target: DirectUploadTarget,
  file: File,
  onProgress: (uploadedBytes: number) => void,
  signal?: AbortSignal,
): Promise<void> {
  return new Promise((resolve, reject) => {
    const xhr = new XMLHttpRequest()
    const cleanup = () => signal?.removeEventListener('abort', abort)
    const abort = () => xhr.abort()
    xhr.open(target.method, target.url, true)
    for (const [name, value] of Object.entries(target.headers)) xhr.setRequestHeader(name, value)
    xhr.upload.onprogress = (event) => {
      if (event.lengthComputable) onProgress(Math.min(event.loaded, file.size))
    }
    xhr.onload = () => {
      cleanup()
      if (xhr.status >= 200 && xhr.status < 300) {
        onProgress(file.size)
        resolve()
      } else {
        reject(new Error(`OSS 直传失败：${xhr.status}`))
      }
    }
    xhr.onerror = () => {
      cleanup()
      reject(new Error('OSS 直传失败，请检查 Bucket CORS 与公网 Endpoint'))
    }
    xhr.onabort = () => {
      cleanup()
      reject(new DOMException('upload cancelled', 'AbortError'))
    }
    signal?.addEventListener('abort', abort, { once: true })
    if (signal?.aborted) return abort()
    xhr.send(file)
  })
}

export async function completeUploadSession(
  uploadId: string,
  token: string,
  content: string,
  replyTo: string,
  isSensitive: boolean,
  signal?: AbortSignal,
): Promise<BroadcastMessage> {
  const response = await request(`/api/attachments/uploads/${encodeURIComponent(uploadId)}/complete`, {
    method: 'POST',
    headers: { ...authHeaders(token), 'Content-Type': 'application/json' },
    body: JSON.stringify({ content, reply_to: replyTo || null, is_sensitive: isSensitive }),
    signal,
  })
  if (!response.ok) throw new Error(`完成上传失败：${response.status}`)
  return storedMessageToBroadcast((await response.json()) as StoredMessage)
}

export async function listUploadSessions(roomId: string, token: string): Promise<AttachmentUploadSession[]> {
  const response = await request(`/api/rooms/${encodeURIComponent(roomId)}/attachments/uploads`, {
    headers: authHeaders(token),
  })
  if (!response.ok) throw new Error(`读取上传会话失败：${response.status}`)
  return response.json()
}

export async function cancelUploadSession(uploadId: string, token: string): Promise<void> {
  await request(`/api/attachments/uploads/${encodeURIComponent(uploadId)}`, {
    method: 'DELETE',
    headers: authHeaders(token),
  })
}

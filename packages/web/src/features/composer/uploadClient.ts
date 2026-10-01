/**
 * Resumable chunked attachment upload for the composer: create the session, PUT the
 * chunks at the server-confirmed offset (`uploadChunks`), complete it with the caption
 * (`uploadAttachment`). The completed
 * message reaches every client (this one included) as a normal `broadcast` frame, so
 * nothing here touches the message store.
 *
 * `@tg/core`'s `ApiClient` only speaks JSON bodies; a chunk is raw bytes, so the
 * transport is an injected fetch (the app passes `browserFetch`, tests a fake). No
 * content hash is sent: it is optional server-side (dedup + direct-to-OSS need it) and
 * hashing a large file in the page before upload would double the wait. See devlog
 * Decisions.
 */
import type { FetchLike, StoredMessage } from '@tg/core'
import { UPLOAD_CHUNK_SIZE } from '@tg/core'
import { t } from '../../i18n/index'

export interface UploadSource {
  name: string
  size: number
  type: string
  lastModified?: number
  slice(start: number, end: number): Blob
}

export interface UploadRequest {
  chatId: string
  token: string
  file: UploadSource
  caption: string
  replyTo: string | null
  /** TG-204: the forum topic to post into; null/absent = General. */
  topicId?: string | null | undefined
  onProgress(uploadedBytes: number, totalBytes: number): void
  signal?: AbortSignal
  chunkSize?: number
}

export class UploadError extends Error {
  constructor(
    message: string,
    readonly status: number,
  ) {
    super(message)
    this.name = 'UploadError'
  }
}

const CREATE_ERRORS: Record<number, string> = {
  get 413() {
    return t('w.composer.f9c82d')
  },
  get 409() {
    return t('w.composer.c34ca0')
  },
  get 403() {
    return t('w.composer.6a85fa')
  },
}

function abortIfNeeded(signal?: AbortSignal): void {
  if (signal?.aborted) {
    const error = new Error('upload cancelled')
    error.name = 'AbortError'
    throw error
  }
}

/** An upload session whose bytes are all on the server, not yet turned into a message. */
export interface UploadedSession {
  uploadId: string
}

export async function uploadAttachment(fetchImpl: FetchLike, request: UploadRequest): Promise<StoredMessage> {
  const { uploadId } = await uploadChunks(fetchImpl, request)
  const auth = { Authorization: `Bearer ${request.token}` }
  abortIfNeeded(request.signal)
  const completed = await fetchImpl(`/api/attachments/uploads/${encodeURIComponent(uploadId)}/complete`, {
    method: 'POST',
    headers: { ...auth, 'Content-Type': 'application/json' },
    cache: 'no-store',
    body: JSON.stringify({
      content: request.caption,
      reply_to: request.replyTo || null,
      is_sensitive: false,
      ...(request.topicId ? { topic_id: request.topicId } : {}),
    }),
    ...(request.signal ? { signal: request.signal } : {}),
  })
  if (!completed.ok) throw new UploadError(t('w.composer.abaa57'), completed.status)
  return (await completed.json()) as StoredMessage
}

/**
 * Create (or resume, by fingerprint) the session and PUT every chunk. TG-403 albums stop
 * here and hand the session ids to `POST /api/chats/:id/albums`, which turns them into
 * messages in one transaction.
 */
export async function uploadChunks(
  fetchImpl: FetchLike,
  request: Omit<UploadRequest, 'caption' | 'replyTo' | 'topicId'>,
): Promise<UploadedSession> {
  const { file, token, signal } = request
  const auth = { Authorization: `Bearer ${token}` }
  const init = (method: string, headers: Record<string, string>, body?: BodyInit): RequestInit => ({
    method,
    headers,
    cache: 'no-store',
    ...(body === undefined ? {} : { body }),
    ...(signal ? { signal } : {}),
  })

  const created = await fetchImpl(
    `/api/chats/${encodeURIComponent(request.chatId)}/attachments/uploads`,
    init(
      'POST',
      { ...auth, 'Content-Type': 'application/json' },
      JSON.stringify({
        file_name: file.name,
        mime_type: file.type || 'application/octet-stream',
        size_bytes: file.size,
        fingerprint: `${file.name}:${file.size}:${file.lastModified ?? 0}`,
      }),
    ),
  )
  if (!created.ok) throw new UploadError(CREATE_ERRORS[created.status] ?? t('w.composer.15ca5e'), created.status)
  const session = (await created.json()) as { upload_id: string; received_bytes: number }
  const uploadPath = `/api/attachments/uploads/${encodeURIComponent(session.upload_id)}`
  const chunkSize = request.chunkSize ?? UPLOAD_CHUNK_SIZE

  let offset = session.received_bytes
  request.onProgress(offset, file.size)
  while (offset < file.size) {
    abortIfNeeded(signal)
    const chunk = file.slice(offset, Math.min(offset + chunkSize, file.size))
    const response = await fetchImpl(
      `${uploadPath}/chunks?offset=${offset}`,
      init('PUT', { ...auth, 'Content-Type': 'application/octet-stream' }, chunk),
    )
    if (response.status === 409) {
      // Offset mismatch: the server tells us where it actually is — resume from there.
      const body = (await response.json()) as { received_bytes?: number }
      const received = body.received_bytes
      if (typeof received !== 'number' || received < 0 || received > file.size || received === offset) {
        throw new UploadError(t('w.composer.907e62'), 409)
      }
      offset = received
    } else if (!response.ok) {
      throw new UploadError(t('w.composer.907e62'), response.status)
    } else {
      offset = ((await response.json()) as { received_bytes: number }).received_bytes
    }
    request.onProgress(offset, file.size)
  }
  return { uploadId: session.upload_id }
}

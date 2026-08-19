import { sha256 } from '@noble/hashes/sha2.js'
import { completeUploadSession, createUploadSession, uploadChunk } from '../api'
import type { BroadcastMessage } from '../types'

export const UPLOAD_CHUNK_SIZE = 4 * 1024 * 1024

export type UploadPhase = 'hashing' | 'uploading' | 'deduplicating' | 'finalizing'

export interface ChunkedUploadProgress {
  uploadId: string
  fileName: string
  phase: UploadPhase
  processedBytes: number
  totalBytes: number
}

export interface ChunkedUploadResult {
  message: BroadcastMessage
  uploadId: string
}

export interface UploadFileOptions {
  roomId: string
  token: string
  password: string
  file: File
  content: string
  replyTo: string
  isSensitive: boolean
  preferredFingerprint?: string
  onProgress: (progress: ChunkedUploadProgress) => void
  onSession?: (uploadId: string) => void | Promise<void>
  signal?: AbortSignal
}

function ensureActive(signal?: AbortSignal): void {
  if (signal?.aborted) throw new DOMException('upload cancelled', 'AbortError')
}

function digestToHex(digest: Uint8Array): string {
  return Array.from(digest, (byte) => byte.toString(16).padStart(2, '0')).join('')
}

export async function hashFile(
  file: File,
  onProgress?: (processedBytes: number) => void,
  signal?: AbortSignal,
): Promise<string> {
  const hasher = sha256.create()
  let offset = 0
  while (offset < file.size) {
    ensureActive(signal)
    const end = Math.min(offset + UPLOAD_CHUNK_SIZE, file.size)
    hasher.update(new Uint8Array(await file.slice(offset, end).arrayBuffer()))
    offset = end
    onProgress?.(offset)
  }
  return digestToHex(hasher.digest())
}

/** Register immediately, hash locally, then resume at the server-confirmed offset. */
export async function uploadFileInChunks(options: UploadFileOptions): Promise<ChunkedUploadResult> {
  const { file, signal } = options
  const report = (phase: UploadPhase, processedBytes: number, uploadId = '') => {
    options.onProgress({
      uploadId,
      fileName: file.name,
      phase,
      processedBytes,
      totalBytes: file.size,
    })
  }
  const fingerprint = options.preferredFingerprint || `${file.name}:${file.size}:${file.lastModified}`
  const initialSession = await createUploadSession(
    options.roomId,
    options.token,
    options.password,
    file,
    fingerprint,
    '',
  )
  await options.onSession?.(initialSession.upload_id)
  report('hashing', 0, initialSession.upload_id)
  const contentHash = await hashFile(file, (bytes) => report('hashing', bytes, initialSession.upload_id), signal)
  ensureActive(signal)

  const session = await createUploadSession(
    options.roomId,
    options.token,
    options.password,
    file,
    fingerprint,
    contentHash,
  )
  let offset = session.received_bytes
  report(session.deduplicated ? 'deduplicating' : 'uploading', offset, session.upload_id)

  while (offset < file.size) {
    ensureActive(signal)
    const chunk = file.slice(offset, Math.min(offset + UPLOAD_CHUNK_SIZE, file.size))
    try {
      offset = await uploadChunk(session.upload_id, options.token, offset, chunk)
    } catch (caught) {
      const receivedBytes = (caught as { receivedBytes?: number }).receivedBytes
      if (typeof receivedBytes === 'number' && receivedBytes >= 0 && receivedBytes <= file.size) {
        offset = receivedBytes
        report('uploading', offset, session.upload_id)
        continue
      }
      throw caught
    }
    report('uploading', offset, session.upload_id)
  }

  report('finalizing', file.size, session.upload_id)
  const message = await completeUploadSession(
    session.upload_id,
    options.token,
    options.content,
    options.replyTo,
    options.isSensitive,
  )
  return { message, uploadId: session.upload_id }
}

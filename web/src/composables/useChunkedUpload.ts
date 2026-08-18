import { completeUploadSession, createUploadSession, uploadChunk } from '../api'
import type { BroadcastMessage } from '../types'

const CHUNK_SIZE = 4 * 1024 * 1024
// Small files aren't worth the extra round-trips a chunked session costs.
export const CHUNKED_UPLOAD_THRESHOLD = 8 * 1024 * 1024

export interface ChunkedUploadProgress {
  fileName: string
  sentBytes: number
  totalBytes: number
}

function fingerprintOf(file: File): string {
  return `${file.name}:${file.size}:${file.lastModified}`
}

/**
 * Upload a large file in resumable chunks. If the exact same file (matched by
 * name+size+lastModified) was already partially uploaded in this room, the
 * server hands back that in-progress session and we continue from its
 * received_bytes instead of restarting — this is what makes retrying after a
 * dropped connection cheap. There's no silent auto-resume across a page
 * reload (the browser doesn't let us keep a File handle across reloads); the
 * user has to re-pick the same file, at which point this same mechanism kicks in.
 */
export async function uploadFileInChunks(
  roomId: string,
  token: string,
  password: string,
  file: File,
  content: string,
  replyTo: string,
  isSensitive: boolean,
  onProgress: (progress: ChunkedUploadProgress) => void,
  signal?: AbortSignal,
): Promise<BroadcastMessage> {
  const fingerprint = fingerprintOf(file)
  const session = await createUploadSession(roomId, token, password, file, fingerprint)
  let offset = session.received_bytes

  while (offset < file.size) {
    if (signal?.aborted) throw new DOMException('upload cancelled', 'AbortError')
    const chunk = file.slice(offset, Math.min(offset + CHUNK_SIZE, file.size))
    try {
      offset = await uploadChunk(session.upload_id, token, offset, chunk)
    } catch (caught) {
      const receivedBytes = (caught as { receivedBytes?: number }).receivedBytes
      if (typeof receivedBytes === 'number') {
        // The server's idea of the offset won the race — resume from there.
        offset = receivedBytes
        continue
      }
      throw caught
    }
    onProgress({ fileName: file.name, sentBytes: offset, totalBytes: file.size })
  }

  return completeUploadSession(session.upload_id, token, content, replyTo, isSensitive)
}

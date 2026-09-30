/**
 * Chunked SHA-256 of a file with progress and abort, migrated from
 * `web/src/composables/useChunkedUpload.ts::hashFile` (TG-011).
 *
 * What migrated is the logic the card wants kept: 4 MiB chunking, per-chunk progress,
 * abort between chunks, lowercase hex digest. The digest primitive and the byte source are
 * injected (`Sha256StreamFactory`, `FileChunkSource` — see `types/platform.ts`), because
 * `@noble/hashes` is outside this package's dependency grant and `File`/`Blob` are host
 * concepts. `web` adapts a `File` via `createFileReader()`; the test injects Bun's hasher.
 */
import type { FileChunkSource, Sha256StreamFactory } from '../types'

export const UPLOAD_CHUNK_SIZE = 4 * 1024 * 1024

export interface HashFileOptions {
  onProgress?: (processedBytes: number) => void
  /** Cooperative cancellation, checked between chunks. */
  isCancelled?: () => boolean
  chunkSize?: number
}

/** `Error` with `name: 'AbortError'` — the property callers branch on (not DOMException). */
export function abortError(message = 'hashing cancelled'): Error {
  const error = new Error(message)
  error.name = 'AbortError'
  return error
}

export async function hashFile(
  source: FileChunkSource,
  createSha256: Sha256StreamFactory,
  options: HashFileOptions = {},
): Promise<string> {
  const chunkSize = options.chunkSize ?? UPLOAD_CHUNK_SIZE
  const stream = createSha256()
  let offset = 0
  while (offset < source.size) {
    if (options.isCancelled?.()) throw abortError()
    const end = Math.min(offset + chunkSize, source.size)
    stream.update(await source.readChunk(offset, end))
    offset = end
    options.onProgress?.(offset)
  }
  return stream.digestHex()
}

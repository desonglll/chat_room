/**
 * Upload progress → percent, per phase. Migrated verbatim from
 * `web/src/attachmentUploadProgress.ts` (TG-011); `UploadPhase` now lives in
 * `domain/messageView.ts` (it is a client view state, not a wire type).
 */
import type { UploadPhase } from './messageView'

export function uploadPercent(phase: UploadPhase, processedBytes: number, totalBytes: number): number {
  const ratio = totalBytes > 0 ? Math.min(1, Math.max(0, processedBytes / totalBytes)) : 0
  if (phase === 'queued') return 0
  if (phase === 'hashing' || phase === 'uploading') return Math.round(ratio * 100)
  return 100
}

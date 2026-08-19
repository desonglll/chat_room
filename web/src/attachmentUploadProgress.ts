import type { UploadPhase } from './types'

export function uploadPercent(phase: UploadPhase, processedBytes: number, totalBytes: number): number {
  const ratio = totalBytes > 0 ? Math.min(1, Math.max(0, processedBytes / totalBytes)) : 0
  if (phase === 'queued') return 0
  if (phase === 'hashing' || phase === 'uploading') return Math.round(ratio * 100)
  return 100
}

import type { AiThreadMessage } from './types'

export type AiUiMessage = AiThreadMessage

export function aiContextUsage(
  totalMessageCount: number | null,
  retrievedMessageCount: number | null,
): { recent: number; retrieved: number } {
  const total = Math.max(0, totalMessageCount || 0)
  const retrieved = Math.min(total, Math.max(0, retrievedMessageCount || 0))
  return { recent: total - retrieved, retrieved }
}

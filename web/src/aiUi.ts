import type { AiCitationSource, AiThreadMessage } from './types'

export type AiUiMessage = AiThreadMessage

export function aiContextUsage(
  totalMessageCount: number | null,
  retrievedMessageCount: number | null,
): { recent: number; retrieved: number } {
  const total = Math.max(0, totalMessageCount || 0)
  const retrieved = Math.min(total, Math.max(0, retrievedMessageCount || 0))
  return { recent: total - retrieved, retrieved }
}

export function aiSourceRoute(source: AiCitationSource) {
  return {
    name: 'room' as const,
    params: { id: source.room_id },
    query: { message: source.message_id },
  }
}

/**
 * Which entry animation a newly appended row gets. Migrated verbatim from
 * `web/src/messageMotion.ts` (TG-011); only the type import path changed.
 */
import type { MessageMotion } from './messageView'

export function classifyMessageMotion(
  historyReady: boolean,
  senderId: string | null,
  currentUserId: string,
): MessageMotion {
  if (!historyReady) return 'none'
  return senderId === currentUserId ? 'outgoing' : 'incoming'
}

export function classifySystemMotion(historyReady: boolean): MessageMotion {
  return historyReady ? 'system' : 'none'
}

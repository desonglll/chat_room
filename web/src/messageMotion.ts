import type { MessageMotion } from './types'

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

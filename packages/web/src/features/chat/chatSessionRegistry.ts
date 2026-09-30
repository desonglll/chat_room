/**
 * The open chat sessions by chat id, so surfaces outside the chat pane (the root-level
 * media viewer, the selection bar) can act on a chat's socket without owning it.
 * `useChatSession` registers while its session is alive; nothing else writes here.
 */
import type { ClientFrame } from '@tg/core'

export interface RegisteredChatSession {
  sendFrame(frame: ClientFrame): boolean
}

const sessions = new Map<string, RegisteredChatSession>()

export function registerChatSession(chatId: string, session: RegisteredChatSession): () => void {
  sessions.set(chatId, session)
  return () => {
    if (sessions.get(chatId) === session) sessions.delete(chatId)
  }
}

/** Send one frame on an open chat's socket; false when the chat is not open or offline. */
export function sendChatFrame(chatId: string, frame: ClientFrame): boolean {
  return sessions.get(chatId)?.sendFrame(frame) ?? false
}

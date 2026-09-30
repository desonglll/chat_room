/**
 * Presence seam for chat rows (TG-107 owns presence). The row takes plain props —
 * `isOnline` and `typingText` — and this adapter is the ONE place that decides where
 * they come from. Today it reads the existing `presenceStore`, which only holds data for
 * a chat whose session is open; TG-107's hooks replace `useChatRowPresence`'s body.
 */
import type { ChatPresence, ConversationSummary } from '@tg/core'
import { presenceStore } from '@tg/core'
import { useStore } from 'zustand/react'

export interface ChatRowPresence {
  isOnline: boolean | undefined
  typingText: string | null
}

const NO_PRESENCE: ChatRowPresence = { isOnline: undefined, typingText: null }

/** Pure derivation, so the seam is testable without a store. */
export function deriveChatRowPresence(
  presence: ChatPresence | undefined,
  conversation: ConversationSummary,
  currentUserId: string,
): ChatRowPresence {
  if (!presence) return NO_PRESENCE
  const peerId = conversation.kind === 'direct' ? (conversation.peer?.id ?? '') : ''
  const isOnline = peerId ? presence.participants.some((member) => member.user_id === peerId) : undefined
  const typists = presence.typing.filter((indicator) => indicator.user_id !== currentUserId)
  let typingText: string | null = null
  if (typists.length > 0) {
    if (conversation.kind === 'direct') typingText = '正在输入…'
    else {
      const names = typists.map((indicator) => indicator.username).filter(Boolean)
      typingText = names.length > 1 ? `${names.length} 人正在输入…` : `${names[0] ?? '有人'} 正在输入…`
    }
  }
  return { isOnline, typingText }
}

export function useChatRowPresence(conversation: ConversationSummary, currentUserId: string): ChatRowPresence {
  const presence = useStore(presenceStore, (state) => state.chats[conversation.room_id])
  return deriveChatRowPresence(presence, conversation, currentUserId)
}

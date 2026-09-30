/**
 * Presence seam for chat rows (TG-107 owns presence). The row takes plain props —
 * `isOnline` and `typingText` — and this adapter is the ONE place that decides where
 * they come from. Today it reads the existing `presenceStore`, which only holds data for
 * a chat whose session is open; Typing text now comes from TG-107's `useTypingSummary`.
 */
import type { ChatPresence, ConversationSummary } from '@tg/core'
import { presenceStore } from '@tg/core'
import { useStore } from 'zustand/react'
import { useTypingSummary } from '../presence'

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
  // TG-107 owns the typing copy (nine actions, 1/2/3+ merging, 5 s expiry); the local
  // derivation still supplies `isOnline` and is the fallback text.
  const typingSummary = useTypingSummary(conversation.room_id)
  const derived = deriveChatRowPresence(presence, conversation, currentUserId)
  return typingSummary === null ? derived : { ...derived, typingText: typingSummary }
}

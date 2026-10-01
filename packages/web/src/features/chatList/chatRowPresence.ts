/**
 * Presence seam for chat rows (TG-107 owns presence). The row takes plain props —
 * `isOnline` and `typingText` — and this adapter is the ONE place that decides where
 * they come from. Typing reads the chat's `presenceStore` entry (only while its session is
 * open); typing text now comes from TG-107's `useTypingSummary`.
 *
 * TG-1208: `isOnline` is the peer's privacy-filtered status (`presenceStore.users`), the same
 * value the chat header shows. It used to be «the peer is in this chat's participant list» —
 * the roster, not who is connected — so an open private chat always showed its peer online
 * (even one hiding last seen), while the header said «离线».
 */
import type { ChatPresence, ConversationSummary, UserStatus } from '@tg/core'
import { presenceStore, selectUserStatus } from '@tg/core'
import { useStore } from 'zustand/react'
import { useTypingSummary } from '../presence'
import { t } from '../../i18n/index'

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
  peerStatus?: UserStatus | undefined,
): ChatRowPresence {
  const peerId = conversation.kind === 'direct' ? (conversation.peer?.id ?? '') : ''
  const isOnline = peerId && peerStatus ? peerStatus.kind === 'online' : undefined
  if (!presence) return { ...NO_PRESENCE, isOnline }
  const typists = presence.typing.filter((indicator) => indicator.user_id !== currentUserId)
  let typingText: string | null = null
  if (typists.length > 0) {
    if (conversation.kind === 'direct') typingText = t('w.chatList.bd19ae')
    else {
      const names = typists.map((indicator) => indicator.username).filter(Boolean)
      typingText =
        names.length > 1
          ? t('w.chatList.8222dd', names.length)
          : t('w.chatList.384831', names[0] ?? t('w.chatList.someone'))
    }
  }
  return { isOnline, typingText }
}

export function useChatRowPresence(conversation: ConversationSummary, currentUserId: string): ChatRowPresence {
  const presence = useStore(presenceStore, (state) => state.chats[conversation.room_id])
  const peerStatus = useStore(presenceStore, selectUserStatus(conversation.peer?.id ?? ''))
  // TG-107 owns the typing copy (nine actions, 1/2/3+ merging, 5 s expiry); the local
  // derivation still supplies `isOnline` and is the fallback text.
  const typingSummary = useTypingSummary(conversation.room_id)
  const derived = deriveChatRowPresence(presence, conversation, currentUserId, peerStatus)
  return typingSummary === null ? derived : { ...derived, typingText: typingSummary }
}

/**
 * Store → `ChatListRow` for one conversation: the draft from `composerStore`, presence
 * through the `useChatRowPresence` seam. One component per row so a typing change in
 * one chat re-renders one row.
 */
import type { MouseEvent } from 'react'
import type { ConversationSummary } from '@tg/core'
import { composerStore } from '@tg/core'
import { useStore } from 'zustand/react'
import { ChatListRow } from './ChatListRow'
import { toChatRowModel } from './chatRowModel'
import { useChatRowPresence } from './chatRowPresence'
import { EmojiStatus } from '../customEmoji/EmojiStatus'

export interface ConnectedChatRowProps {
  conversation: ConversationSummary
  currentUserId: string
  activeChatId: string
  collapsed: boolean
  now: Date
  onOpen: (chatId: string, event: MouseEvent<HTMLAnchorElement>) => void
}

export function ConnectedChatRow({
  conversation,
  currentUserId,
  activeChatId,
  collapsed,
  now,
  onOpen,
}: ConnectedChatRowProps) {
  const draftText = useStore(composerStore, (state) => state.drafts[conversation.room_id]?.text ?? '')
  const presence = useChatRowPresence(conversation, currentUserId)
  const model = toChatRowModel(conversation, { currentUserId, activeChatId, draftText, now })
  return (
    <ChatListRow
      model={model}
      href={`/chat/${encodeURIComponent(conversation.room_id)}`}
      active={conversation.room_id === activeChatId}
      collapsed={collapsed}
      isOnline={presence.isOnline}
      typingText={presence.typingText}
      onOpen={onOpen}
      titleAdornment={conversation.peer ? <EmojiStatus userId={conversation.peer.id} size={16} /> : undefined}
    />
  )
}

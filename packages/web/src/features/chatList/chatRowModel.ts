/**
 * Conversation (wire) → chat row (view). Pure; the row component never reads the wire
 * shape, so TG-208's unification of the list endpoint only has to change this adapter.
 */
import type { ChatType, ConversationSummary } from '@tg/core'
import { conversationChatType, isConversationMuted } from '@tg/core'
import type { ChatPreview } from './chatPreview'
import { buildChatPreview } from './chatPreview'
import { formatChatListTime } from './chatTime'

/** Own-last-message delivery tick: one tick sent, two ticks read by the other side. */
export type OutgoingStatus = 'sent' | 'read'

export interface ChatRowModel {
  chatId: string
  chatType: ChatType
  title: string
  avatarLabel: string
  avatarInitials: string | undefined
  avatarSrc: string | undefined
  preview: ChatPreview
  time: string
  unreadCount: number
  muted: boolean
  pinned: boolean
  outgoing: OutgoingStatus | null
  /** Only a private chat has a single peer whose online dot means anything. */
  showsPresence: boolean
}

export interface RowContext {
  currentUserId: string
  activeChatId: string
  draftText: string
  now: Date
  /** Read-state seam: whether the other side has read `messageId`. No server data yet. */
  isReadByPeer?: ((chatId: string, messageId: string) => boolean) | undefined
}

/** Avatar field: an uploaded avatar is a same-origin `/api/...` URL, otherwise an emoji. */
export function avatarParts(value: string): { src: string | undefined; initials: string | undefined } {
  if (value.startsWith('/api/')) return { src: value, initials: undefined }
  return { src: undefined, initials: value || undefined }
}

export function toChatRowModel(conversation: ConversationSummary, context: RowContext): ChatRowModel {
  const chatType = conversationChatType(conversation)
  const active = conversation.room_id === context.activeChatId
  const last = conversation.last_message
  const title = conversation.alias || conversation.title
  const { src, initials } = avatarParts(conversation.avatar_emoji)
  const own = last !== null && !last.recalled && last.sender_id !== null && last.sender_id === context.currentUserId
  return {
    chatId: conversation.room_id,
    chatType,
    title,
    avatarLabel: title,
    avatarInitials: initials,
    avatarSrc: src,
    preview: buildChatPreview({
      chatType,
      lastMessage: last,
      currentUserId: context.currentUserId,
      draftText: context.draftText,
      active,
    }),
    time: formatChatListTime(last?.created_at ?? conversation.last_activity_at, context.now),
    unreadCount: active ? 0 : conversation.unread_count,
    muted: isConversationMuted(conversation, context.now.getTime()),
    pinned: conversation.preferences.is_pinned && !conversation.preferences.is_archived,
    outgoing: own ? (context.isReadByPeer?.(conversation.room_id, last.message_id) ? 'read' : 'sent') : null,
    showsPresence: chatType === 'private',
  }
}

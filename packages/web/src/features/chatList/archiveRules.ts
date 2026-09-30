/**
 * TG-502 archive rules, pure and framework-free.
 *
 *  - Unarchive on a new message: a non-muted archived chat returns to the main list when
 *    another member's message arrives; a muted one stays archived. The server enforces the
 *    same rule in a trigger (`20270201000009_unarchive_on_new_message.sql`, both adapters), so this
 *    client copy only keeps the live list in step until the next REST resync agrees with it.
 *  - The archive row's badge: the unread sum over archived non-muted chats (accent); when that
 *    is zero, the sum over the muted ones, in grey.
 */
import type { ConversationPreferences, ConversationSummary } from '@tg/core'
import { isConversationMuted } from '@tg/core'

export interface ArchiveBadge {
  count: number
  /** Grey badge: every archived chat with unread messages is muted. */
  muted: boolean
}

const isArchived = (conversation: ConversationSummary) => conversation.preferences.is_archived

export function archiveBadge(conversations: readonly ConversationSummary[], now: number): ArchiveBadge {
  let loud = 0
  let quiet = 0
  for (const conversation of conversations) {
    if (!isArchived(conversation) || conversation.unread_count <= 0) continue
    if (isConversationMuted(conversation, now)) quiet += conversation.unread_count
    else loud += conversation.unread_count
  }
  return loud > 0 ? { count: loud, muted: false } : { count: quiet, muted: true }
}

/**
 * Should a message from `senderId` pull this conversation out of the archive? `null` sender is a
 * system message, which counts like the server's trigger counts it.
 */
export function unarchivesOnMessage(
  conversation: ConversationSummary,
  senderId: string | null,
  currentUserId: string,
  now: number,
): boolean {
  if (!isArchived(conversation)) return false
  if (senderId !== null && senderId === currentUserId) return false
  return !isConversationMuted(conversation, now)
}

/** Replace one conversation's preferences (optimistic update or a server reply). */
export function withPreferences(
  conversations: readonly ConversationSummary[],
  chatId: string,
  change: Partial<ConversationPreferences>,
): ConversationSummary[] {
  return conversations.map((conversation) =>
    conversation.room_id === chatId
      ? { ...conversation, preferences: { ...conversation.preferences, ...change } }
      : conversation,
  )
}

/** The archive row's summary: newest archived chats first (unread ones ahead), for the preview. */
export function archivePreviewChats(
  conversations: readonly ConversationSummary[],
  limit: number,
): ConversationSummary[] {
  const archived = conversations.filter(isArchived)
  const unread = archived.filter((conversation) => conversation.unread_count > 0)
  const read = archived.filter((conversation) => conversation.unread_count <= 0)
  return [...unread, ...read].slice(0, limit)
}

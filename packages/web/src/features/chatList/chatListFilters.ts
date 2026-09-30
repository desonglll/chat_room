/**
 * Which rows the pane shows: the main list or the archive folder, narrowed by the
 * sidebar search box. Pure; the order comes from `sortConversations` in the store.
 */
import type { ConversationSummary } from '@tg/core'
import { sortConversations } from '@tg/core'

export type ChatListFolder = 'main' | 'archive'

export interface ChatListView {
  rows: ConversationSummary[]
  /** Archived chats, for the archive entry row (main folder, no search). */
  archivedCount: number
  archivedUnread: number
}

const matches = (conversation: ConversationSummary, needle: string): boolean =>
  [
    conversation.title,
    conversation.alias,
    conversation.peer?.username ?? '',
    conversation.peer?.display_name ?? '',
  ].some((field) => field.toLocaleLowerCase().includes(needle))

export function selectChatListView(
  conversations: readonly ConversationSummary[],
  folder: ChatListFolder,
  query: string,
): ChatListView {
  const sorted = sortConversations(conversations)
  const archived = sorted.filter((conversation) => conversation.preferences.is_archived)
  const needle = query.trim().toLocaleLowerCase()
  // Search spans both folders, like Telegram's sidebar search.
  const pool = needle ? sorted : folder === 'archive' ? archived : sorted.filter((c) => !c.preferences.is_archived)
  return {
    rows: needle ? pool.filter((conversation) => matches(conversation, needle)) : pool,
    archivedCount: archived.length,
    archivedUnread: archived.reduce((total, conversation) => total + conversation.unread_count, 0),
  }
}

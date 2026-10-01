/**
 * TG-501 chat folders (Telegram's filters). The server stores each folder's rules; membership
 * and unread counts are evaluated HERE, from the same conversation list the sidebar shows, so a
 * folder's unread count is by construction the sum of its chats' unread counts.
 *
 * A chat is in a folder when:
 *   not explicitly excluded,
 *   AND (explicitly included OR its type is one of the folder's types),
 *   AND not removed by an exclusion flag (muted / read / archived).
 * An explicitly included chat still obeys the exclusion flags — Telegram's behaviour: a folder
 * that "excludes read" hides read chats you pinned into it too.
 */
import type { ConversationSummary } from '../stores/chatListStore'

export type FolderChatType = 'private' | 'groups' | 'channels'

export interface ChatFolder {
  id: string
  title: string
  emoji: string
  /** Chat types the folder takes in whole. */
  include_types: FolderChatType[]
  include_chat_ids: string[]
  exclude_chat_ids: string[]
  exclude_muted: boolean
  exclude_read: boolean
  exclude_archived: boolean
}

export function folderChatType(conversation: ConversationSummary): FolderChatType {
  if (conversation.kind === 'direct') return 'private'
  return conversation.group?.chat_type === 'channel' ? 'channels' : 'groups'
}

export function isMuted(conversation: ConversationSummary, now: number): boolean {
  const { notification_level: level, muted_until: until } = conversation.preferences
  return level === 'none' || (until !== null && Date.parse(until) > now)
}

export function inFolder(folder: ChatFolder, conversation: ConversationSummary, now: number): boolean {
  const id = conversation.room_id
  if (folder.exclude_chat_ids.includes(id)) return false
  const matches = folder.include_chat_ids.includes(id) || folder.include_types.includes(folderChatType(conversation))
  if (!matches) return false
  if (folder.exclude_muted && isMuted(conversation, now)) return false
  if (folder.exclude_read && conversation.unread_count === 0) return false
  if (folder.exclude_archived && conversation.preferences.is_archived) return false
  return true
}

export function folderConversations(
  folder: ChatFolder,
  conversations: readonly ConversationSummary[],
  now: number,
): ConversationSummary[] {
  return conversations.filter((conversation) => inFolder(folder, conversation, now))
}

/** Unread messages in the folder (muted chats count, like the sidebar's own badges). */
export function folderUnread(folder: ChatFolder, conversations: readonly ConversationSummary[], now: number): number {
  return folderConversations(folder, conversations, now).reduce(
    (sum, conversation) => sum + conversation.unread_count,
    0,
  )
}

/** Telegram allows 10 folders. */
export const MAX_CHAT_FOLDERS = 10

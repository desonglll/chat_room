/**
 * Which of the three info headers a Chat gets, and what it shows. Pure: the panel feeds
 * it the store rows it already has, so the choice is testable without React.
 *
 * `chat_type` decides the variant, as everywhere else (CONTEXT.md "Chat Type"):
 * `private` → a person, `group`/`supergroup` → a group, `channel` → a channel. Channels
 * arrive in M2; the variant exists now so the panel does not have to be reshaped then.
 */
import type { Chat, ChatType, ConversationSummary, PresenceState, User } from '@tg/core'

export type InfoVariant = 'private' | 'group' | 'channel'

interface HeaderBase {
  title: string
  avatarEmoji: string
  /** `@handle` without the `@`, or empty. */
  username: string
}

export interface PrivateHeader extends HeaderBase {
  variant: 'private'
  /** The peer's user id — the last-seen line reads presence for it. Empty when unknown. */
  userId: string
  /** The peer's profile signature (Telegram's "bio"); empty until the profile loads. */
  bio: string
}

export interface GroupHeader extends HeaderBase {
  variant: 'group'
  memberCount: number
  description: string
}

export interface ChannelHeader extends HeaderBase {
  variant: 'channel'
  subscriberCount: number
  description: string
}

export type InfoHeaderModel = PrivateHeader | GroupHeader | ChannelHeader

export function infoVariant(chatType: ChatType): InfoVariant {
  if (chatType === 'private') return 'private'
  if (chatType === 'channel') return 'channel'
  return 'group'
}

export interface InfoHeaderInput {
  chatId: string
  chat: Chat | null
  conversation: ConversationSummary | null
  /** The peer's public profile (`GET /api/users/:id`), once loaded. */
  peerProfile: User | null
  /** Fallback peer id for a private chat without a conversation row (e.g. from presence). */
  peerIdFallback?: string
}

function chatTypeOf(input: InfoHeaderInput): ChatType | null {
  if (input.chat) return input.chat.chat_type
  if (input.conversation) {
    return input.conversation.kind === 'direct' ? 'private' : (input.conversation.group?.chat_type ?? 'group')
  }
  return null
}

/** Null while neither the descriptor nor the sidebar row is known. */
export function selectInfoHeader(input: InfoHeaderInput): InfoHeaderModel | null {
  const chatType = chatTypeOf(input)
  if (chatType === null) return null
  const { chat, conversation, peerProfile } = input
  const variant = infoVariant(chatType)

  if (variant === 'private') {
    const peer = conversation?.peer ?? null
    const userId = peerProfile?.id ?? peer?.id ?? input.peerIdFallback ?? ''
    const displayName = peerProfile?.display_name || peer?.display_name || ''
    const username = peerProfile?.username ?? peer?.username ?? ''
    return {
      variant,
      userId,
      title: conversation?.alias || displayName || username || conversation?.title || chat?.title || '',
      avatarEmoji: peerProfile?.avatar_emoji || peer?.avatar_emoji || conversation?.avatar_emoji || '',
      username,
      bio: peerProfile?.signature ?? '',
    }
  }

  const title = conversation?.alias || chat?.title || conversation?.title || ''
  const avatarEmoji = chat?.avatar_emoji || conversation?.avatar_emoji || ''
  const description = chat?.description || conversation?.description || ''
  const username = chat?.username ?? conversation?.group?.username ?? ''
  const count = chat?.member_count ?? conversation?.group?.member_count ?? 0
  if (variant === 'channel') {
    return { variant, title, avatarEmoji, username, description, subscriberCount: count }
  }
  return { variant, title, avatarEmoji, username, description, memberCount: count }
}

const COUNT_FORMAT = new Intl.NumberFormat('zh-CN')

/** Same rule as TG-107's header line: the online part only when more than one is online. */
export function memberCountText(count: number, onlineCount = 0): string {
  const base = `${COUNT_FORMAT.format(count)} 位成员`
  return onlineCount > 1 ? `${base}，${COUNT_FORMAT.format(onlineCount)} 人在线` : base
}

export function subscriberCountText(count: number): string {
  return `${COUNT_FORMAT.format(count)} 位订阅者`
}

export const PANEL_HEADING: Record<InfoVariant, string> = {
  private: '用户信息',
  group: '群组信息',
  channel: '频道信息',
}

/**
 * Members the live socket knows about, and how many of them are online by their status
 * (the same source as each member row's "在线", so the header never disagrees with the list).
 */
export function countMembers(state: PresenceState, chatId: string): { members: number; online: number } {
  const members = state.chats[chatId]?.members ?? []
  let online = 0
  for (const member of members) if (state.users[member.user_id]?.kind === 'online') online += 1
  return { members: members.length, online }
}

/**
 * `chats.member_count` is a projection that joins do not maintain yet (TG-201 owns "member_count
 * 投影的事务内维护"), so it can read 1 for a group of six. Never show fewer members than the
 * live snapshot already proves exist.
 */
export function withKnownMembers(header: InfoHeaderModel | null, knownMembers: number): InfoHeaderModel | null {
  if (header?.variant !== 'group' || header.memberCount >= knownMembers) return header
  return { ...header, memberCount: knownMembers }
}

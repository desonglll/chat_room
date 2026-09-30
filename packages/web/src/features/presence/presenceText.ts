/**
 * Pure derivations from the store states to the strings TG-107 renders. Kept apart from
 * the hooks so every rule is testable without a DOM: the hooks only choose WHEN to call
 * these (store change or shared tick), never WHAT they say.
 */
import type { ActiveTypingAction, ChatListState, ChatType, PresenceState, UserStatus } from '@tg/core'
import { TYPING_TTL_MS, formatLastSeen, selectChatById, selectPresence, summarizeTyping } from '@tg/core'

export interface PresenceInputs {
  presence: PresenceState
  chatList: ChatListState
  currentUserId: string
  now: number
}

export type HeaderStatus =
  | { kind: 'typing'; text: string; action: ActiveTypingAction }
  | { kind: 'members'; text: string }
  | { kind: 'last_seen'; text: string; online: boolean }

/** Unknown chats (not in the list yet) are phrased as groups, so names are never lost. */
const chatTypeOf = (inputs: PresenceInputs, chatId: string): ChatType =>
  selectChatById(chatId)(inputs.chatList)?.chat_type ?? 'group'

export function typingSummaryFor(inputs: PresenceInputs, chatId: string) {
  return summarizeTyping({
    indicators: selectPresence(chatId)(inputs.presence).typing,
    now: inputs.now,
    chatType: chatTypeOf(inputs, chatId),
    currentUserId: inputs.currentUserId,
  })
}

/** The shared ticker's wake hint: the next instant any visible typing line expires. */
export function nextTypingExpiry(presence: PresenceState, now: number): number | null {
  let earliest: number | null = null
  for (const chat of Object.values(presence.chats)) {
    for (const indicator of chat.typing) {
      const expiry = indicator.receivedAt + TYPING_TTL_MS
      if (expiry > now && (earliest === null || expiry < earliest)) earliest = expiry
    }
  }
  return earliest
}

export function lastSeenTextFor(status: UserStatus | undefined, now: number): string {
  return formatLastSeen(status, now)
}

/** Telegram header copy for a multi-member chat: "12 位成员，3 人在线" (online only when > 1). */
export function memberCountText(chatType: ChatType, memberCount: number, onlineCount: number): string {
  const noun = chatType === 'channel' ? '位订阅者' : '位成员'
  const base = `${memberCount} ${noun}`
  return chatType !== 'channel' && onlineCount > 1 ? `${base}，${onlineCount} 人在线` : base
}

/** The other side of a private chat, from the chat's live member snapshots. */
export function privatePeerId(inputs: PresenceInputs, chatId: string): string | null {
  const presence = selectPresence(chatId)(inputs.presence)
  const peer = [...presence.participants, ...presence.members].find((member) => member.user_id !== inputs.currentUserId)
  return peer?.user_id ?? null
}

export function headerStatusFor(inputs: PresenceInputs, chatId: string): HeaderStatus {
  const typing = typingSummaryFor(inputs, chatId)
  if (typing) return { kind: 'typing', text: typing.text, action: typing.action }
  const chat = selectChatById(chatId)(inputs.chatList)
  const chatType = chat?.chat_type ?? 'group'
  if (chatType === 'private') {
    const peerId = privatePeerId(inputs, chatId)
    const status = peerId ? inputs.presence.users[peerId] : undefined
    return { kind: 'last_seen', text: lastSeenTextFor(status, inputs.now), online: status?.kind === 'online' }
  }
  const presence = selectPresence(chatId)(inputs.presence)
  const online = new Set(presence.members.map((member) => member.user_id))
  for (const [userId, status] of Object.entries(presence.statuses)) {
    if (status.kind === 'online') online.add(userId)
  }
  return { kind: 'members', text: memberCountText(chatType, chat?.member_count ?? 0, online.size) }
}

export function sameHeaderStatus(a: HeaderStatus, b: HeaderStatus): boolean {
  if (a.kind !== b.kind || a.text !== b.text) return false
  if (a.kind === 'typing' && b.kind === 'typing') return a.action === b.action
  if (a.kind === 'last_seen' && b.kind === 'last_seen') return a.online === b.online
  return true
}

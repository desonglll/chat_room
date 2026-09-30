/**
 * Online members, participants, typing indicators and per-user status tiers, per chat.
 * Applies the WS `presence`, `typing` (all ten actions; `cancel` and the legacy
 * empty-content form both clear) and `user_status` frames. Expiry is driven by the host
 * calling `expireTyping(chatId, now)` — core holds no timers.
 *
 * TG-107: a refreshed indicator keeps its position (display order = first seen, so a
 * multi-user line does not reshuffle on every refresh frame), and the latest status per
 * user is also kept account-wide in `users` for last-seen lookups outside a chat.
 */
import { createStore } from 'zustand/vanilla'
import type { ChatMember, ServerFrame, TypingFrame, UserStatus } from '../types'
import { TYPING_TTL_MS } from '../domain/typingSummary'

export { TYPING_TTL_MS }

export interface TypingIndicator {
  user_id: string
  username: string
  content: string
  action: Exclude<TypingFrame['action'], 'cancel'>
  /** Host clock ms; indicator expires TYPING_TTL_MS after this. */
  receivedAt: number
}

export interface ChatPresence {
  members: ChatMember[]
  participants: ChatMember[]
  typing: TypingIndicator[]
  statuses: Record<string, UserStatus>
}

const EMPTY_PRESENCE: ChatPresence = { members: [], participants: [], typing: [], statuses: {} }

export interface PresenceState {
  chats: Record<string, ChatPresence>
  /** Latest known status per user, across every chat (last write wins). */
  users: Record<string, UserStatus>
  applyPresence(chatId: string, frame: Extract<ServerFrame, { type: 'presence' }>): void
  applyAuthOk(chatId: string, frame: Extract<ServerFrame, { type: 'auth_ok' }>): void
  applyTyping(chatId: string, frame: TypingFrame, receivedAt: number): void
  applyUserStatus(chatId: string, frame: Extract<ServerFrame, { type: 'user_status' }>): void
  expireTyping(chatId: string, now: number): void
  clearChat(chatId: string): void
}

export const createPresenceStore = () =>
  createStore<PresenceState>()((set) => {
    const update = (chatId: string, mutate: (presence: ChatPresence) => ChatPresence) =>
      set((state) => ({ chats: { ...state.chats, [chatId]: mutate(state.chats[chatId] ?? EMPTY_PRESENCE) } }))

    return {
      chats: {},
      users: {},
      applyPresence: (chatId, frame) =>
        update(chatId, (presence) => ({ ...presence, members: frame.members, participants: frame.participants })),
      applyAuthOk: (chatId, frame) => {
        const statuses = Object.fromEntries(frame.statuses.map((entry) => [entry.user_id, entry.status]))
        update(chatId, (presence) => ({
          ...presence,
          members: frame.members,
          participants: frame.participants,
          statuses,
        }))
        set((state) => ({ users: { ...state.users, ...statuses } }))
      },
      applyTyping: (chatId, frame, receivedAt) =>
        update(chatId, (presence) => {
          const userId = frame.user_id ?? ''
          const others = presence.typing.filter((indicator) => indicator.user_id !== userId)
          // `cancel` clears, and so does the legacy form: empty content + plain `typing`.
          if (!userId || frame.action === 'cancel' || (frame.action === 'typing' && !frame.content)) {
            return { ...presence, typing: others }
          }
          const next: TypingIndicator = {
            user_id: userId,
            username: frame.username ?? '',
            content: frame.content,
            action: frame.action,
            receivedAt,
          }
          const index = presence.typing.findIndex((indicator) => indicator.user_id === userId)
          if (index < 0) return { ...presence, typing: [...presence.typing, next] }
          const typing = [...presence.typing]
          typing[index] = next
          return { ...presence, typing }
        }),
      applyUserStatus: (chatId, frame) => {
        update(chatId, (presence) => ({
          ...presence,
          statuses: { ...presence.statuses, [frame.user_id]: frame.status },
        }))
        set((state) => ({ users: { ...state.users, [frame.user_id]: frame.status } }))
      },
      expireTyping: (chatId, now) =>
        update(chatId, (presence) => ({
          ...presence,
          typing: presence.typing.filter((indicator) => now - indicator.receivedAt < TYPING_TTL_MS),
        })),
      clearChat: (chatId) =>
        set((state) => {
          const { [chatId]: _removed, ...rest } = state.chats
          return { chats: rest }
        }),
    }
  })

export type PresenceStore = ReturnType<typeof createPresenceStore>

export const selectPresence = (chatId: string) => (state: PresenceState) => state.chats[chatId] ?? EMPTY_PRESENCE

/** A user's latest status from any chat; `undefined` when never reported. */
export const selectUserStatus = (userId: string) => (state: PresenceState) => state.users[userId]

export const presenceStore = createPresenceStore()

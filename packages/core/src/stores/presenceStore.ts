/**
 * Online members, participants, typing indicators and per-user status tiers, per chat.
 * Applies the WS `presence`, `typing` (all ten actions; `cancel` and the legacy
 * empty-content form both clear) and `user_status` frames. Expiry is driven by the host
 * calling `expireTyping(chatId, now)` — core holds no timers.
 */
import { createStore } from 'zustand/vanilla'
import type { ChatMember, ServerFrame, TypingFrame, UserStatus } from '../types'

export interface TypingIndicator {
  user_id: string
  username: string
  content: string
  action: Exclude<TypingFrame['action'], 'cancel'>
  /** Host clock ms; indicator expires TYPING_TTL_MS after this. */
  receivedAt: number
}

export const TYPING_TTL_MS = 6_000

export interface ChatPresence {
  members: ChatMember[]
  participants: ChatMember[]
  typing: TypingIndicator[]
  statuses: Record<string, UserStatus>
}

const EMPTY_PRESENCE: ChatPresence = { members: [], participants: [], typing: [], statuses: {} }

export interface PresenceState {
  chats: Record<string, ChatPresence>
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
      applyPresence: (chatId, frame) =>
        update(chatId, (presence) => ({ ...presence, members: frame.members, participants: frame.participants })),
      applyAuthOk: (chatId, frame) =>
        update(chatId, (presence) => ({
          ...presence,
          members: frame.members,
          participants: frame.participants,
          statuses: Object.fromEntries(frame.statuses.map((entry) => [entry.user_id, entry.status])),
        })),
      applyTyping: (chatId, frame, receivedAt) =>
        update(chatId, (presence) => {
          const userId = frame.user_id ?? ''
          const others = presence.typing.filter((indicator) => indicator.user_id !== userId)
          // `cancel` clears, and so does the legacy form: empty content + plain `typing`.
          if (!userId || frame.action === 'cancel' || (frame.action === 'typing' && !frame.content)) {
            return { ...presence, typing: others }
          }
          return {
            ...presence,
            typing: [
              ...others,
              {
                user_id: userId,
                username: frame.username ?? '',
                content: frame.content,
                action: frame.action,
                receivedAt,
              },
            ],
          }
        }),
      applyUserStatus: (chatId, frame) =>
        update(chatId, (presence) => ({
          ...presence,
          statuses: { ...presence.statuses, [frame.user_id]: frame.status },
        })),
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

export const presenceStore = createPresenceStore()

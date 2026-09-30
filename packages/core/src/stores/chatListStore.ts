/**
 * The chat list (sidebar source). Skeleton: canonical `Chat` rows keyed by id, plus the
 * `chat_updated` WS application. Sorting/folders/archive arrive with M1 TG-102, which
 * owns this file from then on (one domain, one file).
 */
import { createStore } from 'zustand/vanilla'
import type { Chat } from '../types'

export interface ChatListState {
  chats: Chat[]
  loading: boolean
  error: string
  setChats(chats: Chat[]): void
  upsertChat(chat: Chat): void
  removeChat(chatId: string): void
  /** Apply a WS `chat_updated` frame's descriptor. */
  applyChatUpdated(chat: Chat): void
  setUnreadCount(chatId: string, unreadCount: number): void
  setLoading(loading: boolean): void
  setError(error: string): void
}

export const createChatListStore = () =>
  createStore<ChatListState>()((set) => ({
    chats: [],
    loading: false,
    error: '',
    setChats: (chats) => set({ chats, error: '' }),
    upsertChat: (chat) =>
      set((state) => {
        const index = state.chats.findIndex((existing) => existing.id === chat.id)
        if (index < 0) return { chats: [...state.chats, chat] }
        const chats = [...state.chats]
        chats[index] = chat
        return { chats }
      }),
    removeChat: (chatId) => set((state) => ({ chats: state.chats.filter((chat) => chat.id !== chatId) })),
    applyChatUpdated: (chat) =>
      set((state) => ({
        chats: state.chats.map((existing) => (existing.id === chat.id ? { ...existing, ...chat } : existing)),
      })),
    setUnreadCount: (chatId, unreadCount) =>
      set((state) => ({
        chats: state.chats.map((chat) => (chat.id === chatId ? { ...chat, unread_count: unreadCount } : chat)),
      })),
    setLoading: (loading) => set({ loading }),
    setError: (error) => set({ error }),
  }))

export type ChatListStore = ReturnType<typeof createChatListStore>

export const selectChatById = (chatId: string) => (state: ChatListState) =>
  state.chats.find((chat) => chat.id === chatId) ?? null

export const chatListStore = createChatListStore()

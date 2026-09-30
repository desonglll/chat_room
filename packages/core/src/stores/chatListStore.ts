/**
 * The chat list (sidebar source), owned by TG-102.
 *
 * Two collections, on purpose:
 *  - `chats` — canonical `Chat` descriptors from `GET /api/chats`. The chat header and the
 *    info pane read these (`selectChatById`); `chat_updated` frames patch them.
 *  - `conversations` — the sidebar rows from `GET /api/conversations`: per-user preferences
 *    (pin / archive / mute), the last-message preview and the peer of a private chat. Live
 *    updates arrive from the account socket (`new_message`, `unread_counts`) and from the
 *    open chat timelines (`applyLatestMessage`), because the account socket skips own messages.
 *
 * Sorting is a pure function (`sortConversations`) so the order is testable and identical
 * wherever the list is read. The wire mirrors live here, beside their only store, rather
 * than in `types/` (AGENTS.md: domain types beside the domain).
 */
import { createStore } from 'zustand/vanilla'
import type { Chat, ChatType } from '../types'

export type NotificationLevel = 'all' | 'mentions' | 'none'

/** `src/conversations/models.rs::ConversationPreferences`. */
export interface ConversationPreferences {
  room_id: string
  is_pinned: boolean
  is_archived: boolean
  notification_level: NotificationLevel
  muted_until: string | null
  updated_at: string
}

/** `src/conversations/models.rs::MessagePreview`; `content` is capped at 120 chars server-side. */
export interface ConversationLastMessage {
  message_id: string
  sender_id: string | null
  sender: string
  content: string
  attachment_file_name: string | null
  recalled: boolean
  created_at: string
}

export interface ConversationPeer {
  id: string
  username: string
  avatar_emoji: string
  display_name: string
}

/** One sidebar row as `GET /api/conversations` serialises it (read-only use of that route). */
export interface ConversationSummary {
  room_id: string
  kind: 'group' | 'direct'
  title: string
  alias: string
  avatar_emoji: string
  description: string
  /** The chat descriptor for non-private chats; carries `chat_type`. */
  group: (Chat & { name?: string }) | null
  peer: ConversationPeer | null
  unread_count: number
  pending_join_requests: number
  preferences: ConversationPreferences
  last_message: ConversationLastMessage | null
  last_activity_at: string
  created_at: string
}

/** Account socket `new_message` (`src/accounts/account_events.rs`). Never the caller's own. */
export interface AccountMessageEvent {
  type: 'new_message'
  message_id: string
  room_id: string
  conversation_kind: 'group' | 'direct'
  conversation_title: string
  sender_id: string | null
  sender: string
  content: string
  attachment_file_name: string | null
  timestamp: string
  is_mention: boolean
}

/** One entry of the account socket `unread_counts` snapshot. */
export interface AccountChatState {
  room_id: string
  unread_count: number
  membership_status: string
}

export interface ChatListState {
  chats: Chat[]
  conversations: ConversationSummary[]
  loading: boolean
  error: string
  setChats(chats: Chat[]): void
  upsertChat(chat: Chat): void
  removeChat(chatId: string): void
  /** Apply a WS `chat_updated` frame's descriptor. */
  applyChatUpdated(chat: Chat): void
  setUnreadCount(chatId: string, unreadCount: number): void
  setConversations(conversations: ConversationSummary[]): void
  /** Another member's message in any chat; `activeChatId` does not accumulate unread. */
  applyAccountMessage(event: AccountMessageEvent, activeChatId: string): void
  /** The account socket's full snapshot: unread counts, and drops non-active memberships. */
  applyUnreadCounts(states: AccountChatState[]): void
  /**
   * The newest message an open chat timeline knows (own sends included — the account
   * socket never echoes them). Replaces the same message (recall/edit) or a newer one;
   * never touches the unread count.
   */
  applyLatestMessage(chatId: string, message: ConversationLastMessage): void
  setLoading(loading: boolean): void
  setError(error: string): void
}

const mapConversation = (
  conversations: ConversationSummary[],
  chatId: string,
  change: (conversation: ConversationSummary) => ConversationSummary,
): ConversationSummary[] =>
  conversations.map((conversation) => (conversation.room_id === chatId ? change(conversation) : conversation))

const isNewer = (candidate: string, current: ConversationLastMessage | null): boolean =>
  current === null || Date.parse(candidate) >= Date.parse(current.created_at)

export const createChatListStore = () =>
  createStore<ChatListState>()((set) => ({
    chats: [],
    conversations: [],
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
    removeChat: (chatId) =>
      set((state) => ({
        chats: state.chats.filter((chat) => chat.id !== chatId),
        conversations: state.conversations.filter((conversation) => conversation.room_id !== chatId),
      })),
    applyChatUpdated: (chat) =>
      set((state) => ({
        chats: state.chats.map((existing) => (existing.id === chat.id ? { ...existing, ...chat } : existing)),
        conversations: mapConversation(state.conversations, chat.id, (conversation) =>
          conversation.kind === 'group'
            ? { ...conversation, title: chat.title, avatar_emoji: chat.avatar_emoji, group: chat }
            : conversation,
        ),
      })),
    setUnreadCount: (chatId, unreadCount) =>
      set((state) => ({
        chats: state.chats.map((chat) => (chat.id === chatId ? { ...chat, unread_count: unreadCount } : chat)),
        conversations: mapConversation(state.conversations, chatId, (conversation) => ({
          ...conversation,
          unread_count: unreadCount,
        })),
      })),
    setConversations: (conversations) => set({ conversations, error: '' }),
    applyAccountMessage: (event, activeChatId) =>
      set((state) => ({
        conversations: mapConversation(state.conversations, event.room_id, (conversation) => {
          const current = conversation.last_message
          if (current?.message_id === event.message_id || !isNewer(event.timestamp, current)) return conversation
          return {
            ...conversation,
            unread_count: activeChatId === event.room_id ? 0 : conversation.unread_count + 1,
            last_activity_at: event.timestamp,
            last_message: {
              message_id: event.message_id,
              sender_id: event.sender_id,
              sender: event.sender,
              content: event.content,
              attachment_file_name: event.attachment_file_name,
              recalled: false,
              created_at: event.timestamp,
            },
          }
        }),
      })),
    applyUnreadCounts: (states) =>
      set((state) => {
        const byId = new Map(states.map((entry) => [entry.room_id, entry]))
        return {
          conversations: state.conversations
            .filter((conversation) => {
              const entry = byId.get(conversation.room_id)
              return entry === undefined || entry.membership_status === 'active'
            })
            .map((conversation) => {
              const entry = byId.get(conversation.room_id)
              return entry === undefined || entry.unread_count === conversation.unread_count
                ? conversation
                : { ...conversation, unread_count: entry.unread_count }
            }),
        }
      }),
    applyLatestMessage: (chatId, message) =>
      set((state) => ({
        conversations: mapConversation(state.conversations, chatId, (conversation) => {
          const current = conversation.last_message
          const same = current?.message_id === message.message_id
          if (!same && !isNewer(message.created_at, current)) return conversation
          if (same && current.recalled === message.recalled && current.content === message.content) {
            return conversation
          }
          return { ...conversation, last_message: message, last_activity_at: message.created_at }
        }),
      })),
    setLoading: (loading) => set({ loading }),
    setError: (error) => set({ error }),
  }))

export type ChatListStore = ReturnType<typeof createChatListStore>

export const selectChatById = (chatId: string) => (state: ChatListState) =>
  state.chats.find((chat) => chat.id === chatId) ?? null

/** The conversation's `chat_type`: a direct row is `private`, a group row reads its descriptor. */
export const conversationChatType = (conversation: ConversationSummary): ChatType =>
  conversation.kind === 'direct' ? 'private' : (conversation.group?.chat_type ?? 'group')

/** Muted = notifications off, or a `muted_until` still in the future. */
export function isConversationMuted(conversation: ConversationSummary, now: number): boolean {
  if (conversation.preferences.notification_level === 'none') return true
  const until = conversation.preferences.muted_until
  return until !== null && Date.parse(until) > now
}

/**
 * Telegram order: pinned first (in the main list only — archived rows ignore pins), then
 * newest activity, then id as a stable tie-break. Archived rows go last.
 */
export function sortConversations(conversations: readonly ConversationSummary[]): ConversationSummary[] {
  return [...conversations].sort((left, right) => {
    const archived = Number(left.preferences.is_archived) - Number(right.preferences.is_archived)
    if (archived !== 0) return archived
    if (!left.preferences.is_archived) {
      const pinned = Number(right.preferences.is_pinned) - Number(left.preferences.is_pinned)
      if (pinned !== 0) return pinned
    }
    const activity = Date.parse(right.last_activity_at) - Date.parse(left.last_activity_at)
    if (activity !== 0 && !Number.isNaN(activity)) return activity
    return left.room_id.localeCompare(right.room_id)
  })
}

export const chatListStore = createChatListStore()

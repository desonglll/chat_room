/**
 * Per-chat message timelines: history, live merge (optimistic reconciliation + dedup),
 * edits, recalls, reactions, delivery states, read cursors. The mutation logic itself
 * lives in `domain/` (migrated with tests); this store is the shard map around it.
 *
 * TG-100: older REST pages the virtual list loads are PREPENDED here (`prependHistory`)
 * instead of living in the list's private state, so every live frame (edit, recall,
 * reaction) reaches every loaded row — the store timeline is the one loaded window of the
 * live view. Other members' read cursors (`auth_ok.read_receipts` + `read_receipt`) live
 * here too; `peerReadThrough` turns them into the outgoing double tick.
 */
import { createStore } from 'zustand/vanilla'
import type { ReadReceipt, ServerFrame, StoredMessage } from '../types'
import { mergeIncomingBroadcast } from '../domain/chatIncoming'
import { createOptimisticMessage, updateDeliveryState } from '../domain/chatOptimistic'
import { applyMessageReaction } from '../domain/messageReactions'
import type { BroadcastMessage, DisplayMessage, MessageMotion } from '../domain/messageView'
import { storedMessageToBroadcast } from '../domain/messageView'

export interface ChatTimeline {
  messages: DisplayMessage[]
  historyReady: boolean
  /** user_id → the newest message_id that user has read (server read cursor). */
  readCursors: Record<string, string>
}

const EMPTY_TIMELINE: ChatTimeline = { messages: [], historyReady: false, readCursors: {} }

export interface MessageState {
  timelines: Record<string, ChatTimeline>
  /** Replace a chat's history from REST (`StoredMessage[]`, oldest first). */
  setHistory(chatId: string, history: StoredMessage[]): void
  setHistoryReady(chatId: string, ready: boolean): void
  /**
   * Put an older REST page (any order, may overlap) in front of the timeline. Only
   * messages strictly older than the oldest server message already present are taken
   * (so a page can never punch into or duplicate the loaded range); returns how many
   * were added. A page that adds nothing leaves the state untouched (same identity).
   */
  prependHistory(chatId: string, page: readonly BroadcastMessage[]): number
  /** Replace/advance read cursors (the `auth_ok` snapshot or one `read_receipt` frame). */
  applyReadReceipts(chatId: string, receipts: readonly Pick<ReadReceipt, 'user_id' | 'message_id'>[]): void
  /** Merge one live/replayed/caught-up broadcast; returns the acknowledged client id. */
  applyBroadcast(chatId: string, incoming: BroadcastMessage, motion: MessageMotion): string
  appendOptimistic(chatId: string, input: Omit<Parameters<typeof createOptimisticMessage>[0], 'messages'>): void
  markDelivery(chatId: string, clientMessageId: string, state: 'sending' | 'failed'): void
  applyEdit(chatId: string, frame: Extract<ServerFrame, { type: 'message_edited' }>): void
  applyRecall(chatId: string, frame: Extract<ServerFrame, { type: 'message_recalled' }>): void
  /** TG-405: drop messages deleted for everyone (auto-delete). */
  applyDeleted(chatId: string, frame: Extract<ServerFrame, { type: 'messages_deleted' }>): void
  applyReaction(chatId: string, frame: Extract<ServerFrame, { type: 'reaction_changed' }>): void
  clearChat(chatId: string): void
}

type TimelineUpdate = (timeline: ChatTimeline) => ChatTimeline

export const createMessageStore = () =>
  createStore<MessageState>()((set) => {
    const update = (chatId: string, mutate: TimelineUpdate) =>
      set((state) => ({
        timelines: { ...state.timelines, [chatId]: mutate(state.timelines[chatId] ?? EMPTY_TIMELINE) },
      }))

    return {
      timelines: {},
      setHistory: (chatId, history) =>
        update(chatId, (timeline) => ({
          ...timeline,
          messages: history.map((message) => storedMessageToBroadcast(message)),
        })),
      setHistoryReady: (chatId, ready) => update(chatId, (timeline) => ({ ...timeline, historyReady: ready })),
      prependHistory: (chatId, page) => {
        let added = 0
        update(chatId, (timeline) => {
          const older = olderThanLoaded(timeline.messages, page)
          added = older.length
          if (added === 0) return timeline
          return { ...timeline, messages: (older as DisplayMessage[]).concat(timeline.messages) }
        })
        return added
      },
      applyReadReceipts: (chatId, receipts) => {
        if (receipts.length === 0) return
        update(chatId, (timeline) => {
          const readCursors = { ...timeline.readCursors }
          for (const receipt of receipts) readCursors[receipt.user_id] = receipt.message_id
          return { ...timeline, readCursors }
        })
      },
      applyBroadcast: (chatId, incoming, motion) => {
        let acknowledged = ''
        update(chatId, (timeline) => {
          const result = mergeIncomingBroadcast(timeline.messages, incoming, motion)
          acknowledged = result.acknowledgedClientId
          return { ...timeline, messages: result.messages }
        })
        return acknowledged
      },
      appendOptimistic: (chatId, input) =>
        update(chatId, (timeline) => ({
          ...timeline,
          messages: [...timeline.messages, createOptimisticMessage({ ...input, messages: timeline.messages })],
        })),
      markDelivery: (chatId, clientMessageId, state) =>
        update(chatId, (timeline) => ({
          ...timeline,
          messages: updateDeliveryState(timeline.messages, clientMessageId, state),
        })),
      applyEdit: (chatId, frame) =>
        update(chatId, (timeline) => ({
          ...timeline,
          messages: timeline.messages.map((message) =>
            message.type === 'broadcast' && message.message_id === frame.message_id
              ? { ...message, content: frame.content, edited_at: frame.edited_at }
              : message,
          ),
        })),
      applyRecall: (chatId, frame) =>
        update(chatId, (timeline) => ({
          ...timeline,
          messages: timeline.messages.map((message) =>
            message.type === 'broadcast' && message.message_id === frame.message_id
              ? { ...message, recalled_at: frame.recalled_at }
              : message,
          ),
        })),
      applyDeleted: (chatId, frame) =>
        update(chatId, (timeline) => {
          const gone = new Set(frame.message_ids)
          return {
            ...timeline,
            messages: timeline.messages.filter(
              (message) => message.type !== 'broadcast' || !gone.has(message.message_id),
            ),
          }
        }),
      applyReaction: (chatId, frame) =>
        update(chatId, (timeline) => ({ ...timeline, messages: applyMessageReaction(timeline.messages, frame) })),
      clearChat: (chatId) =>
        set((state) => {
          const { [chatId]: _removed, ...rest } = state.timelines
          return { timelines: rest }
        }),
    }
  })

export type MessageStore = ReturnType<typeof createMessageStore>

function byTimeThenId(left: BroadcastMessage, right: BroadcastMessage): number {
  const delta = Date.parse(left.timestamp) - Date.parse(right.timestamp)
  if (delta !== 0 && !Number.isNaN(delta)) return delta
  return left.message_id < right.message_id ? -1 : left.message_id > right.message_id ? 1 : 0
}

const isServerMessage = (message: DisplayMessage): message is BroadcastMessage =>
  message.type === 'broadcast' && !message.message_id.startsWith('pending:') && message.delivery_state !== 'sending'

/**
 * The part of `page` that goes in front of `messages`: deduplicated, chronological, and
 * strictly older than the oldest server message present. Pending optimistic rows are
 * always the newest, so they never bound the page. O(page), never O(timeline).
 */
function olderThanLoaded(messages: readonly DisplayMessage[], page: readonly BroadcastMessage[]): BroadcastMessage[] {
  const oldest = messages.find(isServerMessage)
  const unique = new Map<string, BroadcastMessage>()
  for (const message of page) {
    if (unique.has(message.message_id)) continue
    if (oldest && byTimeThenId(message, oldest) >= 0) continue
    unique.set(message.message_id, { ...message, reactions: message.reactions || [] })
  }
  return [...unique.values()].sort(byTimeThenId)
}

/**
 * The newest `timestamp` any OTHER member has read through, among loaded messages
 * ('' when none is known). An outgoing message at or before it shows the double tick.
 * A cursor on a message outside the loaded window is older than all of it (the live
 * window always reaches the newest message), so it marks nothing loaded as read.
 */
export function peerReadThrough(timeline: ChatTimeline, viewerId: string): string {
  const cursorIds = new Set<string>()
  for (const [userId, messageId] of Object.entries(timeline.readCursors)) {
    if (userId !== viewerId) cursorIds.add(messageId)
  }
  if (cursorIds.size === 0) return ''
  let newest = ''
  let newestMs = Number.NEGATIVE_INFINITY
  for (let index = timeline.messages.length - 1; index >= 0; index -= 1) {
    const message = timeline.messages[index]
    if (message?.type !== 'broadcast' || !cursorIds.has(message.message_id)) continue
    const ms = Date.parse(message.timestamp)
    if (ms > newestMs) {
      newestMs = ms
      newest = message.timestamp
    }
    cursorIds.delete(message.message_id)
    if (cursorIds.size === 0) break
  }
  return newest
}

export const selectTimeline = (chatId: string) => (state: MessageState) => state.timelines[chatId] ?? EMPTY_TIMELINE

export const messageStore = createMessageStore()

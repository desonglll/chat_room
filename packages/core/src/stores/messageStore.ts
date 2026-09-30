/**
 * Per-chat message timelines: history, live merge (optimistic reconciliation + dedup),
 * edits, recalls, reactions, delivery states. The mutation logic itself lives in
 * `domain/` (migrated with tests); this store is the shard map around it.
 */
import { createStore } from 'zustand/vanilla'
import type { ServerFrame, StoredMessage } from '../types'
import { mergeIncomingBroadcast } from '../domain/chatIncoming'
import { createOptimisticMessage, updateDeliveryState } from '../domain/chatOptimistic'
import { applyMessageReaction } from '../domain/messageReactions'
import type { BroadcastMessage, DisplayMessage, MessageMotion } from '../domain/messageView'
import { storedMessageToBroadcast } from '../domain/messageView'

export interface ChatTimeline {
  messages: DisplayMessage[]
  historyReady: boolean
}

const EMPTY_TIMELINE: ChatTimeline = { messages: [], historyReady: false }

export interface MessageState {
  timelines: Record<string, ChatTimeline>
  /** Replace a chat's history from REST (`StoredMessage[]`, oldest first). */
  setHistory(chatId: string, history: StoredMessage[]): void
  setHistoryReady(chatId: string, ready: boolean): void
  /** Merge one live/replayed/caught-up broadcast; returns the acknowledged client id. */
  applyBroadcast(chatId: string, incoming: BroadcastMessage, motion: MessageMotion): string
  appendOptimistic(chatId: string, input: Omit<Parameters<typeof createOptimisticMessage>[0], 'messages'>): void
  markDelivery(chatId: string, clientMessageId: string, state: 'sending' | 'failed'): void
  applyEdit(chatId: string, frame: Extract<ServerFrame, { type: 'message_edited' }>): void
  applyRecall(chatId: string, frame: Extract<ServerFrame, { type: 'message_recalled' }>): void
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

export const selectTimeline = (chatId: string) => (state: MessageState) => state.timelines[chatId] ?? EMPTY_TIMELINE

export const messageStore = createMessageStore()

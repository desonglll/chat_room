/**
 * Per-chat composer state: draft text, reply/edit targets, pending attachments.
 *
 * The draft slice is shaped by TG-008's frozen `draft_updated` frame
 * (`{user_id, text, reply_to_message_id, topic_id, updated_at}` — docs/devlog/TG-007.md §3):
 * `applyDraftUpdated` consumes that frame verbatim so cross-device sync needs no adapter.
 * The REST draft client itself is TG-008's module (`api/drafts.ts`) — deliberately not
 * imported here; TG-012 wires the two together.
 */
import { createStore } from 'zustand/vanilla'
import type { DraftUpdatedFrame } from '../types'

export interface ComposerDraft {
  text: string
  replyToMessageId: string | null
  topicId: string | null
  /** Server timestamp of the newest applied sync; '' for local-only edits. */
  updatedAt: string
}

export const EMPTY_DRAFT: ComposerDraft = { text: '', replyToMessageId: null, topicId: null, updatedAt: '' }

export interface ComposerState {
  drafts: Record<string, ComposerDraft>
  /** message_id being edited, per chat. */
  editing: Record<string, string>
  setDraftText(chatId: string, text: string): void
  setReplyTarget(chatId: string, messageId: string | null): void
  setTopic(chatId: string, topicId: string | null): void
  /** Apply a WS `draft_updated` frame; ignored when older than the local sync state. */
  applyDraftUpdated(chatId: string, frame: DraftUpdatedFrame): void
  startEditing(chatId: string, messageId: string): void
  stopEditing(chatId: string): void
  clearDraft(chatId: string): void
}

export const createComposerStore = () =>
  createStore<ComposerState>()((set) => {
    const patch = (chatId: string, change: Partial<ComposerDraft>) =>
      set((state) => ({
        drafts: { ...state.drafts, [chatId]: { ...(state.drafts[chatId] ?? EMPTY_DRAFT), ...change } },
      }))

    return {
      drafts: {},
      editing: {},
      setDraftText: (chatId, text) => patch(chatId, { text }),
      setReplyTarget: (chatId, messageId) => patch(chatId, { replyToMessageId: messageId }),
      setTopic: (chatId, topicId) => patch(chatId, { topicId }),
      applyDraftUpdated: (chatId, frame) =>
        set((state) => {
          const current = state.drafts[chatId] ?? EMPTY_DRAFT
          if (current.updatedAt && frame.updated_at <= current.updatedAt) return {}
          return {
            drafts: {
              ...state.drafts,
              [chatId]: {
                text: frame.text,
                replyToMessageId: frame.reply_to_message_id,
                topicId: frame.topic_id,
                updatedAt: frame.updated_at,
              },
            },
          }
        }),
      startEditing: (chatId, messageId) => set((state) => ({ editing: { ...state.editing, [chatId]: messageId } })),
      stopEditing: (chatId) =>
        set((state) => {
          const { [chatId]: _removed, ...rest } = state.editing
          return { editing: rest }
        }),
      clearDraft: (chatId) =>
        set((state) => {
          const { [chatId]: _removed, ...rest } = state.drafts
          return { drafts: rest }
        }),
    }
  })

export type ComposerStore = ReturnType<typeof createComposerStore>

export const selectDraft = (chatId: string) => (state: ComposerState) => state.drafts[chatId] ?? EMPTY_DRAFT

export const composerStore = createComposerStore()

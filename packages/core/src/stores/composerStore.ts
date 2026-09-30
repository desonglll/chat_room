/**
 * Per-chat composer state: draft text + reply target (synced, TG-008), and the local
 * edit / forward layers of TG-104's bar state machine (`domain/composerMode.ts`).
 * Pending attachments hold host file handles and stay in the web feature, not here.
 *
 * The draft slice is shaped by TG-008's frozen `draft_updated` frame
 * (`{user_id, text, reply_to_message_id, topic_id, updated_at}` — docs/devlog/TG-007.md §3):
 * `applyDraftUpdated` consumes that frame verbatim so cross-device sync needs no adapter.
 * The REST draft client itself is TG-008's module (`api/drafts.ts`) — deliberately not
 * imported here; TG-012 wires the two together.
 */
import { createStore } from 'zustand/vanilla'
import type { DraftUpdatedFrame } from '../types'
import type { ComposerModeEvent, ComposerModeState, EditLayer, ForwardLayer } from '../domain/composerMode'
import { transitionComposerMode } from '../domain/composerMode'
import type { ReplyQuote } from '../types'

export interface ComposerDraft {
  text: string
  replyToMessageId: string | null
  topicId: string | null
  /** Server timestamp of the newest applied sync; '' for local-only edits. */
  updatedAt: string
}

export const EMPTY_DRAFT: ComposerDraft = { text: '', replyToMessageId: null, topicId: null, updatedAt: '' }

/** TG-409: a reply to a message in another chat, as this chat's reply bar shows it. */
export interface ReplySource {
  chatId: string
  chatTitle: string
  sender: string
  text: string
}

/**
 * TG-409: what a reply carries beyond its target — a quoted slice and/or a source chat. Kept
 * beside the drafts (not in them): the cloud draft (TG-008) syncs only the target, and these
 * drop whenever the target changes.
 */
export interface ReplyExtras {
  quote?: ReplyQuote | undefined
  source?: ReplySource | undefined
}

export interface ComposerState {
  drafts: Record<string, ComposerDraft>
  /** The edit layer per chat: the message being edited and the edit text (never synced). */
  editing: Record<string, EditLayer>
  /** Messages queued to be forwarded into this chat. */
  forwarding: Record<string, ForwardLayer>
  /** TG-409: the current reply's quote / source chat, per chat. */
  replyExtras: Record<string, ReplyExtras>
  /** TG-409: reply to `messageId` with a quote and/or from another chat (sets the target too). */
  setReplyWithExtras(chatId: string, messageId: string, extras: ReplyExtras): void
  setDraftText(chatId: string, text: string): void
  setReplyTarget(chatId: string, messageId: string | null): void
  setTopic(chatId: string, topicId: string | null): void
  /** Apply a WS `draft_updated` frame; ignored when older than the local sync state. */
  applyDraftUpdated(chatId: string, frame: DraftUpdatedFrame): void
  /** Run one reply/edit/forward state-machine event; returns the resulting mode. */
  dispatchMode(chatId: string, event: ComposerModeEvent): ComposerModeState
  startEditing(chatId: string, messageId: string, text?: string): void
  stopEditing(chatId: string): void
  clearDraft(chatId: string): void
}

export const createComposerStore = () =>
  createStore<ComposerState>()((set, get) => {
    const patch = (chatId: string, change: Partial<ComposerDraft>) =>
      set((state) => ({
        drafts: { ...state.drafts, [chatId]: { ...(state.drafts[chatId] ?? EMPTY_DRAFT), ...change } },
      }))

    const without = <T>(map: Record<string, T>, chatId: string): Record<string, T> => {
      const { [chatId]: _removed, ...rest } = map
      return rest
    }

    const dispatchMode = (chatId: string, event: ComposerModeEvent): ComposerModeState => {
      const before = selectComposerMode(chatId)(get())
      const after = transitionComposerMode(before, event)
      if (after === before) return before
      set((state) => ({
        replyExtras:
          after.replyToMessageId === before.replyToMessageId ? state.replyExtras : without(state.replyExtras, chatId),
        drafts:
          after.replyToMessageId === before.replyToMessageId
            ? state.drafts
            : {
                ...state.drafts,
                [chatId]: { ...(state.drafts[chatId] ?? EMPTY_DRAFT), replyToMessageId: after.replyToMessageId },
              },
        editing: after.edit ? { ...state.editing, [chatId]: after.edit } : without(state.editing, chatId),
        forwarding: after.forward
          ? { ...state.forwarding, [chatId]: after.forward }
          : without(state.forwarding, chatId),
      }))
      return after
    }

    return {
      drafts: {},
      editing: {},
      forwarding: {},
      replyExtras: {},
      dispatchMode,
      setReplyWithExtras: (chatId, messageId, extras) => {
        dispatchMode(chatId, { type: 'reply', messageId })
        set((state) => ({ replyExtras: { ...state.replyExtras, [chatId]: extras } }))
      },
      setDraftText: (chatId, text) => patch(chatId, { text }),
      setReplyTarget: (chatId, messageId) => {
        patch(chatId, { replyToMessageId: messageId })
        set((state) => ({ replyExtras: without(state.replyExtras, chatId) }))
      },
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
      startEditing: (chatId, messageId, text = '') => {
        dispatchMode(chatId, { type: 'edit', messageId, text })
      },
      stopEditing: (chatId) => set((state) => ({ editing: without(state.editing, chatId) })),
      clearDraft: (chatId) =>
        set((state) => {
          const { [chatId]: _removed, ...rest } = state.drafts
          return { drafts: rest, replyExtras: without(state.replyExtras, chatId) }
        }),
    }
  })

export type ComposerStore = ReturnType<typeof createComposerStore>

export const selectDraft = (chatId: string) => (state: ComposerState) => state.drafts[chatId] ?? EMPTY_DRAFT

/**
 * The chat's bar state, assembled from the three slices. Builds a new object: use it in
 * logic, or pair it with a shallow-equality hook — never as a raw `useStore` selector.
 */
export const selectComposerMode =
  (chatId: string) =>
  (state: ComposerState): ComposerModeState => ({
    replyToMessageId: state.drafts[chatId]?.replyToMessageId ?? null,
    edit: state.editing[chatId] ?? null,
    forward: state.forwarding[chatId] ?? null,
  })

export const composerStore = createComposerStore()

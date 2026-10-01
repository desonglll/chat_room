/**
 * The composer's behaviour without React: what a keystroke, Enter, Escape, "reply",
 * "edit" and "forward" do to the stores, the draft sync and the socket. `Composer.tsx`
 * is a thin view over this, and `test/composerController.test.ts` drives it with fakes.
 *
 * Seams (all injected):
 * - `session` — the chat session's send / draft / raw-frame operations (features/chat);
 * - `composer`, `messages` — the core stores (the edit bar reads the original text);
 * - `actions` — TG-107's `createChatActionSender`: `typing` while the draft changes,
 *   `uploading_*` while files go out, `cancel` on send and on clearing the input;
 * - `forward` — `forwardMessages` bound to the token.
 */
import type {
  ChatActionSender,
  ClientFrame,
  ComposerModeEvent,
  ComposerStore,
  MessageStore,
  AttachmentKind,
  BroadcastMessage,
  MessageEntity,
} from '@tg/core'
import { takeDismissal } from '../linkPreview/linkPreviewStore'
import { createEntityDraft } from '../customEmoji/useEntityDraft'
import type { PickedCustomEmoji } from '../customEmoji/CustomEmojiGrid'
import { EMPTY_DRAFT, editIsDirty, selectComposerMode, uploadChatAction } from '@tg/core'

/** The slice of the chat session the composer needs. */
export interface ComposerSessionApi {
  /** Optimistic append + WS send of the current draft (reply target read from the store). */
  sendMessage(
    text: string,
    options?: { silent?: boolean; noLinkPreview?: boolean; entities?: MessageEntity[] },
  ): boolean
  /** Draft text → store + debounced cloud save (TG-008). */
  setDraftText(text: string): void
  /** One raw client frame on the chat socket: `edit` and `typing` go through here. */
  sendFrame(frame: ClientFrame): boolean
}

export type SubmitOutcome = 'sent' | 'edited' | 'unchanged' | 'forwarded' | 'empty' | 'offline'

export interface ComposerControllerDeps {
  chatId: string
  session: ComposerSessionApi
  composer: ComposerStore
  messages: MessageStore
  actions: ChatActionSender
  forward(messageIds: readonly string[], targetChatId: string): Promise<unknown>
}

export interface ComposerController {
  /** What the input shows: the edit text while editing, else the draft. */
  text(): string
  input(text: string): void
  /**
   * TG-1206: put a custom emoji (its fallback character plus an entity) over `selection`
   * of the draft. Returns the new text and caret, or null while editing (an edit frame
   * keeps the plain text only), where the host inserts the fallback character instead.
   */
  insertCustomEmoji(
    selection: { start: number; end: number },
    emoji: PickedCustomEmoji,
  ): { text: string; caret: number } | null
  submit(): SubmitOutcome
  /** TG-404 «静默发送»: send the draft now without notifications (not while editing). */
  submitSilent(): SubmitOutcome
  /** TG-404 «定时发送»: the draft as a scheduled payload, or null when there is nothing to schedule. */
  schedulePayload(): { content: string; replyTo: string | null } | null
  /** TG-404: the draft became a scheduled message — clear it (and its reply bar) like a send. */
  scheduled(): void
  reply(messageId: string): void
  edit(messageId: string): boolean
  forward(messageIds: readonly string[], fromChatId: string): void
  /** Escape / ✕ on the bar: peels one layer; false when there was nothing to cancel. */
  cancel(): boolean
  /** Media went out as a reply: the reply bar is used up (edit / forward layers stay). */
  consumeReply(): void
  uploadStarted(kind: AttachmentKind): void
  uploadFinished(): void
}

export function findMessage(messages: MessageStore, chatId: string, messageId: string): BroadcastMessage | null {
  const list = messages.getState().timelines[chatId]?.messages ?? []
  for (const message of list) {
    if (message.type === 'broadcast' && message.message_id === messageId) return message
  }
  return null
}

export function createComposerController(deps: ComposerControllerDeps): ComposerController {
  const { chatId, session, composer, actions } = deps
  const draft = () => composer.getState().drafts[chatId] ?? EMPTY_DRAFT
  const mode = () => selectComposerMode(chatId)(composer.getState())
  // TG-1206: the custom emoji ranges of the draft. Local only: a cloud draft synced from
  // another device arrives as plain text, and `forSend` reconciles against whatever is there.
  const entityDraft = createEntityDraft()

  /** Run a bar event; when the synced reply target moved, push it through the draft sync. */
  function dispatch(event: ComposerModeEvent): void {
    const before = draft().replyToMessageId
    composer.getState().dispatchMode(chatId, event)
    if (draft().replyToMessageId !== before) session.setDraftText(draft().text)
  }

  function sendDraft(text: string, silent = false): SubmitOutcome {
    const forwarding = mode().forward
    const { text: content, entities } = entityDraft.forSend(text)
    if (!content && !forwarding) return 'empty'
    let outcome: SubmitOutcome = 'forwarded'
    if (content) {
      // Telegram order: the comment first, then the forwarded messages under it.
      // TG-408: a link card dismissed in the composer is not built for this message.
      const noLinkPreview = takeDismissal(chatId, content)
      const options = {
        ...(silent ? { silent: true } : {}),
        ...(noLinkPreview ? { noLinkPreview: true } : {}),
        ...(entities.length ? { entities } : {}),
      }
      const sent = Object.keys(options).length ? session.sendMessage(content, options) : session.sendMessage(content)
      outcome = sent ? 'sent' : 'offline'
    }
    if (forwarding) void deps.forward(forwarding.messageIds, chatId).catch(() => undefined)
    entityDraft.reset()
    composer.getState().dispatchMode(chatId, { type: 'sent' })
    actions.sendChatAction(chatId, 'cancel')
    return outcome
  }

  return {
    text: () => mode().edit?.text ?? draft().text,

    input(text) {
      if (mode().edit) {
        composer.getState().dispatchMode(chatId, { type: 'editText', text })
        return
      }
      entityDraft.sync(text)
      session.setDraftText(text)
      // Empty text is the stop signal: the sender turns it into one `cancel` frame.
      actions.sendChatAction(chatId, 'typing', text)
    },

    insertCustomEmoji(selection, emoji) {
      if (mode().edit) return null
      const result = entityDraft.insert(draft().text, selection, emoji)
      session.setDraftText(result.text)
      actions.sendChatAction(chatId, 'typing', result.text)
      return { text: result.text, caret: result.caret }
    },

    submit() {
      const edit = mode().edit
      if (!edit) return sendDraft(draft().text)
      if (!editIsDirty(edit)) {
        composer.getState().dispatchMode(chatId, { type: 'sent' })
        return 'unchanged'
      }
      const sent = session.sendFrame({ type: 'edit', message_id: edit.messageId, content: edit.text.trim() })
      if (!sent) return 'offline'
      composer.getState().dispatchMode(chatId, { type: 'sent' })
      return 'edited'
    },

    submitSilent() {
      if (mode().edit) return 'empty'
      return sendDraft(draft().text, true)
    },

    schedulePayload() {
      const current = mode()
      const content = draft().text.trim()
      if (current.edit || current.forward || !content) return null
      return { content, replyTo: current.replyToMessageId ?? null }
    },

    scheduled() {
      entityDraft.reset()
      session.setDraftText('')
      dispatch({ type: 'sent' })
      actions.sendChatAction(chatId, 'cancel')
    },

    reply: (messageId) => dispatch({ type: 'reply', messageId }),

    edit(messageId) {
      const message = findMessage(deps.messages, chatId, messageId)
      if (!message || message.recalled_at) return false
      dispatch({ type: 'edit', messageId, text: message.content })
      return true
    },

    forward: (messageIds, fromChatId) => dispatch({ type: 'forward', messageIds, fromChatId }),

    cancel() {
      const before = mode()
      if (!before.edit && !before.forward && !before.replyToMessageId) return false
      dispatch({ type: 'cancel' })
      return true
    },

    consumeReply() {
      const current = mode()
      if (current.replyToMessageId && !current.edit && !current.forward) dispatch({ type: 'cancel' })
    },

    uploadStarted: (kind) => actions.sendChatAction(chatId, uploadChatAction(kind)),
    uploadFinished: () => actions.sendChatAction(chatId, 'cancel'),
  }
}

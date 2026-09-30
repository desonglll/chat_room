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
} from '@tg/core'
import { EMPTY_DRAFT, editIsDirty, selectComposerMode, uploadChatAction } from '@tg/core'

/** The slice of the chat session the composer needs. */
export interface ComposerSessionApi {
  /** Optimistic append + WS send of the current draft (reply target read from the store). */
  sendMessage(text: string): boolean
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
  submit(): SubmitOutcome
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

  /** Run a bar event; when the synced reply target moved, push it through the draft sync. */
  function dispatch(event: ComposerModeEvent): void {
    const before = draft().replyToMessageId
    composer.getState().dispatchMode(chatId, event)
    if (draft().replyToMessageId !== before) session.setDraftText(draft().text)
  }

  function sendDraft(text: string): SubmitOutcome {
    const forwarding = mode().forward
    const content = text.trim()
    if (!content && !forwarding) return 'empty'
    let outcome: SubmitOutcome = 'forwarded'
    if (content) {
      // Telegram order: the comment first, then the forwarded messages under it.
      outcome = session.sendMessage(content) ? 'sent' : 'offline'
    }
    if (forwarding) void deps.forward(forwarding.messageIds, chatId).catch(() => undefined)
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
      session.setDraftText(text)
      // Empty text is the stop signal: the sender turns it into one `cancel` frame.
      actions.sendChatAction(chatId, 'typing', text)
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

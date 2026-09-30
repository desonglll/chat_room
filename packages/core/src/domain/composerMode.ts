/**
 * TG-104: the composer's reply / edit / forward state machine.
 *
 * The three bars above the input are layers, not one exclusive enum, because Telegram
 * restores what was underneath when a layer is cancelled:
 *
 *   edit      overlays everything. Entering it hides the draft (text + reply target) and
 *             shows the message being edited; cancelling or saving it brings the untouched
 *             draft back. The edit text therefore lives HERE, never in the synced draft.
 *   forward   replaces a reply (Telegram Desktop shows one of the two, not both).
 *   reply     the draft's own `reply_to_message_id`, synced to other devices (TG-008).
 *
 * Every function is pure and total: an event that makes no sense in the current state
 * returns the state unchanged (same reference), so callers can cheaply skip re-renders.
 */

export interface EditLayer {
  messageId: string
  /** The message text when editing began — an unchanged save sends nothing. */
  originalText: string
  /** What the input shows while editing. */
  text: string
}

export interface ForwardLayer {
  messageIds: readonly string[]
  fromChatId: string
}

export interface ComposerModeState {
  replyToMessageId: string | null
  edit: EditLayer | null
  forward: ForwardLayer | null
}

export type ComposerModeEvent =
  | { type: 'reply'; messageId: string }
  | { type: 'edit'; messageId: string; text: string }
  | { type: 'editText'; text: string }
  | { type: 'forward'; messageIds: readonly string[]; fromChatId: string }
  /** Escape / the bar's ✕: peels exactly one layer, topmost first. */
  | { type: 'cancel' }
  /** The composer submitted: an edit closes the edit layer only; a send consumes the draft's bars. */
  | { type: 'sent' }

export type ComposerBar =
  | { kind: 'none' }
  | { kind: 'reply'; messageId: string }
  | { kind: 'edit'; messageId: string }
  | { kind: 'forward'; messageIds: readonly string[]; fromChatId: string }

export const IDLE_COMPOSER_MODE: ComposerModeState = Object.freeze({
  replyToMessageId: null,
  edit: null,
  forward: null,
})

export function transitionComposerMode(state: ComposerModeState, event: ComposerModeEvent): ComposerModeState {
  switch (event.type) {
    case 'reply':
      if (!event.messageId) return state
      // Replying from inside an edit abandons the edit (the draft underneath comes back).
      return { replyToMessageId: event.messageId, edit: null, forward: null }
    case 'edit':
      if (!event.messageId) return state
      return {
        ...state,
        edit: { messageId: event.messageId, originalText: event.text, text: event.text },
      }
    case 'editText':
      if (!state.edit || state.edit.text === event.text) return state
      return { ...state, edit: { ...state.edit, text: event.text } }
    case 'forward':
      if (event.messageIds.length === 0) return state
      return {
        replyToMessageId: null,
        edit: null,
        forward: { messageIds: [...event.messageIds], fromChatId: event.fromChatId },
      }
    case 'cancel':
      if (state.edit) return { ...state, edit: null }
      if (state.forward) return { ...state, forward: null }
      if (state.replyToMessageId) return { ...state, replyToMessageId: null }
      return state
    case 'sent':
      if (state.edit) return { ...state, edit: null }
      if (!state.forward && !state.replyToMessageId) return state
      return IDLE_COMPOSER_MODE
  }
}

/** The one bar shown above the input: edit beats forward beats reply. */
export function activeComposerBar(state: ComposerModeState): ComposerBar {
  if (state.edit) return { kind: 'edit', messageId: state.edit.messageId }
  if (state.forward) return { kind: 'forward', ...state.forward }
  if (state.replyToMessageId) return { kind: 'reply', messageId: state.replyToMessageId }
  return { kind: 'none' }
}

/** Whether saving the edit would change anything (trimmed, like the server compares). */
export function editIsDirty(edit: EditLayer): boolean {
  return edit.text.trim() !== edit.originalText.trim() && edit.text.trim() !== ''
}

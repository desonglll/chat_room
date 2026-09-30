/**
 * TG-104 composer — public surface for the chat pane.
 *
 * `ChatPane` renders `<Composer chatId currentUserId members session />`; `session` is
 * the chat session's `{ sendMessage, setDraftText, sendFrame }` (see devlog TG-104
 * «Integration patch list» for the one-line `sendFrame` addition to chatSession).
 * Other features drive the bars through the controller-free store API:
 * `composerStore.getState().dispatchMode(chatId, { type: 'reply' | 'edit' | 'forward', … })`.
 */
export { Composer } from './Composer'
export type { ComposerProps } from './Composer'
export type { ComposerSessionApi, ComposerController, SubmitOutcome } from './composerController'
export { createComposerController } from './composerController'

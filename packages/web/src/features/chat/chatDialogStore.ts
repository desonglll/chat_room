/**
 * The chat's app-wide dialogs as state (TG-100): the forward target picker and the delete
 * confirmation. Feature-local vanilla stores (pure UI state), opened from bubbles, the
 * selection bar and the media viewer alike; `ChatOverlays` renders them once in the shell.
 */
import { createStore } from 'zustand/vanilla'
import { sendChatFrame } from './chatSessionRegistry'

export interface ForwardRequest {
  fromChatId: string
  messageIds: string[]
  /** TG-409: pick a chat to reply to this message from, instead of forwarding. */
  replyTo?: { messageId: string; sender: string; text: string } | undefined
}

export interface DeleteRequest {
  chatId: string
  messageIds: string[]
  resolve(deleted: boolean): void
}

export interface ChatDialogsState {
  forward: ForwardRequest | null
  remove: DeleteRequest | null
}

export const chatDialogsStore = createStore<ChatDialogsState>()(() => ({ forward: null, remove: null }))

/** Open the forward picker; the composer of the picked chat then carries the forward bar. */
export function requestForward(fromChatId: string, messageIds: string[]): void {
  if (messageIds.length === 0) return
  chatDialogsStore.setState({ forward: { fromChatId, messageIds } })
}

/** TG-409: the same chat picker, to reply to `source` from another chat. */
export function requestReplyElsewhere(
  fromChatId: string,
  source: { messageId: string; sender: string; text: string },
): void {
  chatDialogsStore.setState({ forward: { fromChatId, messageIds: [source.messageId], replyTo: source } })
}

export function closeForward(): void {
  chatDialogsStore.setState({ forward: null })
}

/**
 * Ask before recalling; resolves true once the recall frames went out (the server's
 * `message_recalled` frames then update every open client, this one included).
 */
export function requestDelete(chatId: string, messageIds: string[]): Promise<boolean> {
  if (messageIds.length === 0) return Promise.resolve(false)
  chatDialogsStore.getState().remove?.resolve(false)
  return new Promise((resolve) => chatDialogsStore.setState({ remove: { chatId, messageIds, resolve } }))
}

export function settleDelete(confirmed: boolean): void {
  const request = chatDialogsStore.getState().remove
  if (!request) return
  chatDialogsStore.setState({ remove: null })
  if (!confirmed) return request.resolve(false)
  let sent = true
  for (const messageId of request.messageIds) {
    sent = sendChatFrame(request.chatId, { type: 'recall', message_id: messageId }) && sent
  }
  request.resolve(sent)
}

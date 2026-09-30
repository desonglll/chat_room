/**
 * Binds TG-103's per-message `MessageActions` to this chat (TG-100). Framework-free so
 * `test/messageActions.test.ts` pins the permission rules: an action the viewer may not
 * perform is left undefined, which hides its menu row / button in the bubble.
 *
 * Rules (mirroring the server, `src/messages/actions.rs` + `src/messages/pins.rs`):
 * - edit / delete (recall): own messages only; edit needs text to edit and is never offered
 *   on a poll (the server refuses it: the question lives in the poll, not in `content`);
 * - pin: private chats, or owner/admin role (`message.pin` permission);
 * - reply / forward / react / pin need a server id — the bubble already hides them on
 *   pending rows, the binder simply offers them;
 * - system and upload rows get no actions.
 */
import type { ClientFrame, DisplayMessage, ComposerModeEvent } from '@tg/core'
import type { DeliveryStatus, MessageActions } from '../message'
import { rowMessageIds } from '../album/albumCollapse'

export interface MessageActionDeps {
  chatId: string
  viewerId: string
  canPin: boolean
  sendFrame(frame: ClientFrame): boolean
  dispatchMode(chatId: string, event: ComposerModeEvent): unknown
  toggleSelected(messageId: string): void
  openMedia(attachmentId: string): void
  jumpTo(messageId: string): void
  requestDelete(messageIds: string[]): void
  requestForward(messageIds: string[]): void
  pin(messageId: string): void
  copy(text: string): void
}

const NO_ACTIONS: MessageActions = {}

export function bindMessageActions(message: DisplayMessage, deps: MessageActionDeps): MessageActions {
  if (message.type !== 'broadcast') return NO_ACTIONS
  const id = message.message_id
  const own = message.sender_id !== null && message.sender_id === deps.viewerId
  const server = !id.startsWith('pending:')
  const text = message.content
  // TG-403: an album row stands for all of its items — delete, forward and select act on
  // every item; reply, react, pin and the caption edit address the first (Telegram).
  const ids = rowMessageIds(message)
  const viewerReacted = (emoji: string) =>
    message.reactions.some((reaction) => reaction.emoji === emoji && reaction.user_ids.includes(deps.viewerId))

  return {
    onReply: () => deps.dispatchMode(deps.chatId, { type: 'reply', messageId: id }),
    onEdit:
      own && server && text.trim() !== '' && !message.poll
        ? () => deps.dispatchMode(deps.chatId, { type: 'edit', messageId: id, text })
        : undefined,
    onDelete: own && server ? () => deps.requestDelete(ids) : undefined,
    onForward: server ? () => deps.requestForward(ids) : undefined,
    onReact: server
      ? (emoji) => deps.sendFrame({ type: 'reaction', message_id: id, emoji, active: !viewerReacted(emoji) })
      : undefined,
    onPin: deps.canPin && server ? () => deps.pin(id) : undefined,
    onCopy: text.trim() !== '' ? () => deps.copy(text) : undefined,
    onSelect: server ? () => ids.forEach((itemId) => deps.toggleSelected(itemId)) : undefined,
    onOpenMedia: (attachmentId) => deps.openMedia(attachmentId),
    onJumpTo: (messageId) => deps.jumpTo(messageId),
  }
}

/**
 * The delivery the bubble shows: `read` once another member's read cursor is at or past
 * an outgoing, acknowledged message; otherwise undefined (the bubble falls back to the
 * message's own `delivery_state`).
 */
export function deliveryFor(message: DisplayMessage, viewerId: string, peerReadAt: string): DeliveryStatus | undefined {
  if (message.type !== 'broadcast' || message.sender_id !== viewerId || !peerReadAt) return undefined
  if (message.delivery_state === 'sending' || message.delivery_state === 'failed') return undefined
  return Date.parse(message.timestamp) <= Date.parse(peerReadAt) ? 'read' : undefined
}

/** Whether every selected message may be deleted by the viewer (the bar's delete button). */
export function canDeleteAll(messages: readonly DisplayMessage[], selected: ReadonlySet<string>, viewerId: string) {
  if (selected.size === 0) return false
  let found = 0
  for (const message of messages) {
    if (message.type !== 'broadcast' || !selected.has(message.message_id)) continue
    if (message.sender_id !== viewerId || message.recalled_at !== null) return false
    found += 1
  }
  return found === selected.size
}

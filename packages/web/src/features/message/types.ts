/**
 * The TG-103 public contract: what a message row needs to know about its neighbours
 * (`MessageRenderContext`) and what it can ask the chat session to do (`MessageActions`).
 * The render context is declared once, by the list that computes it (TG-101's
 * `messageList/renderContract.ts`), and re-exported here (unified by TG-100); the actions
 * are declared here, beside the bubble, because they are this feature's domain types.
 */
import type { DisplayMessage } from '@tg/core'
import type { MessageRenderContext } from '../messageList/renderContract'

export type { MessageRenderContext }

/** Where a row sits inside a run of consecutive messages from one sender. */
export type GroupPosition = MessageRenderContext['groupPosition']

/**
 * Per-message callbacks, already bound to the message by the caller. Every member is
 * optional: an absent callback hides the corresponding menu entry / button instead of
 * rendering a dead control. That is also how permissions reach the bubble — the session
 * leaves `onEdit` undefined for a message the viewer may not edit.
 */
export interface MessageActions {
  onReply?: (() => void) | undefined
  onEdit?: (() => void) | undefined
  onDelete?: (() => void) | undefined
  onForward?: (() => void) | undefined
  /** Toggle the viewer's reaction `emoji` on this message. */
  onReact?: ((emoji: string) => void) | undefined
  onPin?: (() => void) | undefined
  /** TG-901: set instead of `onPin` when the message is already pinned. */
  onUnpin?: (() => void) | undefined
  onCopy?: (() => void) | undefined
  /** Toggle this message's selection; the first call enters selection mode. */
  onSelect?: (() => void) | undefined
  onOpenMedia?: ((attachmentId: string) => void) | undefined
  /** Scroll to and highlight another message (reply quote click). */
  onJumpTo?: ((messageId: string) => void) | undefined
  /** TG-409: reply quoting the text selected in this message (a plain reply without one). */
  onQuote?: (() => void) | undefined
  /** TG-409: reply to this message from another chat (opens a chat picker). */
  onReplyElsewhere?: (() => void) | undefined
  /** TG-409: open the source of a cross-chat reply (the viewer may lack access). */
  onOpenReplySource?: ((chatId: string, messageId: string) => void) | undefined
}

/**
 * Delivery as the bubble shows it. `read` cannot be derived from the message alone (read
 * receipts are a separate stream), so the caller passes it; without it the bubble falls
 * back to `message.delivery_state`.
 */
export type DeliveryStatus = 'sending' | 'sent' | 'read' | 'failed'

export interface MessageBubbleProps {
  message: DisplayMessage
  ctx: MessageRenderContext
  actions?: MessageActions | undefined
  /** Viewer id, so a reaction chip the viewer chose renders as chosen. */
  viewerId?: string | undefined
  delivery?: DeliveryStatus | undefined
  /** The list is in selection mode: every row shows a checkbox and a click selects. */
  selectionMode?: boolean | undefined
  /**
   * Keep the avatar column even when this row shows no avatar, so a group's bubbles stay
   * aligned with the one that does. Defaults to `showAvatar`.
   */
  reserveAvatar?: boolean | undefined
}

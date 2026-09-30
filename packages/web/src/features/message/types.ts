/**
 * The TG-103 public contract: what a message row needs to know about its neighbours
 * (`MessageRenderContext`, frozen with TG-101) and what it can ask the chat session to do
 * (`MessageActions`). Both are declared here, beside the bubble, because they are domain
 * types of this feature and not wire types (AGENTS.md "Ownership").
 */
import type { DisplayMessage } from '@tg/core'

/**
 * Where a row sits inside a run of consecutive messages from one sender. TG-101 computes it;
 * the bubble only renders it. Structurally identical to TG-101's declaration — the
 * integration lead unifies the two.
 */
export type GroupPosition = 'single' | 'first' | 'middle' | 'last'

export interface MessageRenderContext {
  groupPosition: GroupPosition
  isOutgoing: boolean
  showAvatar: boolean
  showSenderName: boolean
  highlighted: boolean
  selected: boolean
}

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
  onCopy?: (() => void) | undefined
  /** Toggle this message's selection; the first call enters selection mode. */
  onSelect?: (() => void) | undefined
  onOpenMedia?: ((attachmentId: string) => void) | undefined
  /** Scroll to and highlight another message (reply quote click). */
  onJumpTo?: ((messageId: string) => void) | undefined
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

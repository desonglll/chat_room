/**
 * The seam between the virtual list (TG-101) and whoever draws a message (TG-103's bubble,
 * or `DefaultMessage` until it lands). FROZEN: the list computes every field below from
 * the loaded window; renderers only read them. System and upload rows go through the same
 * `renderMessage` — `message.type` tells them apart.
 */
import type { ReactNode } from 'react'
import type { DisplayMessage } from '@tg/core'

export interface MessageRenderContext {
  groupPosition: 'single' | 'first' | 'middle' | 'last' // same sender, consecutive, within a time window
  isOutgoing: boolean
  showAvatar: boolean // true only on the last message of an incoming group in group chats
  showSenderName: boolean // true on the first message of an incoming group in group chats
  highlighted: boolean // jump-to-message flash
  selected: boolean // selection mode
}

export type RenderMessage = (message: DisplayMessage, ctx: MessageRenderContext) => ReactNode

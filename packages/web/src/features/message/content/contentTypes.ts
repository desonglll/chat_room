/**
 * The plug-in contract between the bubble and whatever draws a message's body. Later tasks
 * (stickers, voice, polls, locations, albums …) implement `MessageContentProps` and register
 * a kind; they never edit the bubble.
 */
import type { ComponentType, ReactNode } from 'react'
import type { BroadcastMessage } from '@tg/core'
import type { MessageActions, MessageRenderContext } from '../types'

export interface MessageContentProps {
  message: BroadcastMessage
  ctx: MessageRenderContext
  actions: MessageActions
  /**
   * The invisible clone of the timestamp/ticks. A content that ends in text MUST render it
   * inline at the very end of that text — it reserves exactly the room the real, absolutely
   * positioned meta needs, so the two can never overlap (Telegram's own trick). `null` when
   * the bubble placed the meta elsewhere (over media, or on the reaction row).
   */
  metaSpacer: ReactNode
}

/**
 * How the bubble frames a content:
 *  - `bubble` — a normal padded bubble with a tail;
 *  - `media`  — borderless (no fill, no padding, no tail) when nothing else shares the
 *               bubble; it falls back to `bubble` as soon as a header, quote or reaction
 *               row is present, with the media drawn full-bleed;
 *  - `bare`   — no bubble at all (stickers, round videos): the content draws everything.
 */
export type ContentFrame = 'bubble' | 'media' | 'bare'

/**
 * Where the timestamp goes when no reaction row claims it: `inline` (end of the text,
 * via `metaSpacer`) or `overlay` (a dark pill over the bottom-right of the media).
 */
export type MetaPlacement = 'inline' | 'overlay'

export interface ContentKind {
  kind: string
  component: ComponentType<MessageContentProps>
  match: (message: BroadcastMessage) => boolean
  frame: (message: BroadcastMessage) => ContentFrame
  metaPlacement: (message: BroadcastMessage) => MetaPlacement
  /**
   * The content starts with a full-bleed medium, so the bubble drops its top padding when
   * nothing (name, forward header, quote) sits above it, and its bottom padding when the
   * meta is overlaid on that medium.
   */
  leadingMedia: boolean
  /** Higher wins; on a tie, the later registration wins (so an override replaces a built-in). */
  priority: number
}

export interface RegisterOptions {
  /** Defaults to matching a future wire discriminator `message.kind === kind`. */
  match?: ((message: BroadcastMessage) => boolean) | undefined
  frame?: ContentFrame | ((message: BroadcastMessage) => ContentFrame) | undefined
  metaPlacement?: MetaPlacement | ((message: BroadcastMessage) => MetaPlacement) | undefined
  leadingMedia?: boolean | undefined
  priority?: number | undefined
}

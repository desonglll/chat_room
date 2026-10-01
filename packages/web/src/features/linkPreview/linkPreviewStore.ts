/**
 * TG-408: link cards that arrived after their message (`link_preview_updated`; `null` = hidden),
 * and the composer's dismissed link per chat (the next send goes out with `no_link_preview`).
 */
import { createStore } from 'zustand/vanilla'
import type { LinkPreview, ServerFrame } from '@tg/core'
import { firstLink } from '@tg/core'

export interface LinkPreviewState {
  cards: Record<string, LinkPreview | null>
  dismissed: Record<string, string>
}

export const linkPreviewStore = createStore<LinkPreviewState>()(() => ({ cards: {}, dismissed: {} }))

export function applyLinkPreviewFrame(frame: Extract<ServerFrame, { type: 'link_preview_updated' }>): void {
  linkPreviewStore.setState((state) => ({ cards: { ...state.cards, [frame.message_id]: frame.preview } }))
}

/** What a bubble shows: a frame's card (or its removal) wins over the message snapshot. */
export function effectiveCard(fromMessage: LinkPreview | undefined, held: LinkPreview | null | undefined) {
  return held === undefined ? (fromMessage ?? null) : held
}

export function dismissComposerLink(chatId: string, link: string): void {
  linkPreviewStore.setState((state) => ({ dismissed: { ...state.dismissed, [chatId]: link } }))
}

/** Whether `text`'s link was dismissed in this chat's composer (consumed by the send). */
export function takeDismissal(chatId: string, text: string): boolean {
  const link = firstLink(text)
  const dismissed = linkPreviewStore.getState().dismissed[chatId]
  if (dismissed === undefined) return false
  linkPreviewStore.setState((state) => {
    const rest = { ...state.dismissed }
    delete rest[chatId]
    return { dismissed: rest }
  })
  return link !== null && link === dismissed
}

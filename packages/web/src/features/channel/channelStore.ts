/**
 * Live channel post fields, keyed by message id: the newest view count any source reported
 * (history, broadcast, a `message_views_updated` frame, a view report's answer) and the
 * post's signature. Counts only ever grow, so an older snapshot never rolls a newer one back.
 *
 * Feature-local on purpose (one domain, one store file, the TG-406 poll-store pattern): the
 * chat session feeds it through `applyViewsFrame`.
 */
import { createStore } from 'zustand/vanilla'
import type { MessageViewCount, ServerFrame } from '@tg/core'

export interface ChannelStoreState {
  views: Record<string, number>
  authors: Record<string, string>
  mergeViews(views: readonly MessageViewCount[]): void
  rememberAuthor(messageId: string, author: string): void
  clear(): void
}

export const createChannelStore = () =>
  createStore<ChannelStoreState>()((set) => ({
    views: {},
    authors: {},
    mergeViews: (incoming) =>
      set((state) => {
        let next: Record<string, number> | null = null
        for (const { message_id: id, views } of incoming) {
          if ((state.views[id] ?? -1) >= views) continue
          next ??= { ...state.views }
          next[id] = views
        }
        return next ? { views: next } : state
      }),
    rememberAuthor: (messageId, author) =>
      set((state) =>
        !author || state.authors[messageId] === author ? state : { authors: { ...state.authors, [messageId]: author } },
      ),
    clear: () => set({ views: {}, authors: {} }),
  }))

export type ChannelStore = ReturnType<typeof createChannelStore>

export const channelStore = createChannelStore()

/** Feed one `message_views_updated` frame. Wired in the chat session's frame fan-out. */
export function applyViewsFrame(
  frame: Extract<ServerFrame, { type: 'message_views_updated' }>,
  store: ChannelStore = channelStore,
): void {
  store.getState().mergeViews(frame.views)
}

/** What a post shows: the larger of its own snapshot and anything newer. */
export function effectiveViews(fromMessage: number, held: number | undefined): number {
  return held === undefined ? fromMessage : Math.max(fromMessage, held)
}

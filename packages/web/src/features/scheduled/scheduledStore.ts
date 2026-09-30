/**
 * TG-404: the viewer's own scheduled messages, per chat. Feature-local (one domain, one store
 * file). A scheduled message's id becomes the delivered message's id, so `dropDelivered`
 * removes entries as soon as their broadcast reaches the timeline.
 */
import { createStore } from 'zustand/vanilla'
import type { ScheduledMessage } from '@tg/core'

export interface ScheduledStoreState {
  /** Undefined = not loaded yet for that chat. */
  byChat: Record<string, ScheduledMessage[] | undefined>
  replace(chatId: string, items: ScheduledMessage[]): void
  upsert(item: ScheduledMessage): void
  remove(chatId: string, id: string): void
  /** Drop every entry whose id now exists as a delivered message. */
  dropDelivered(chatId: string, messageIds: ReadonlySet<string>): void
  clear(): void
}

const byTime = (a: ScheduledMessage, b: ScheduledMessage): number =>
  a.scheduled_at === b.scheduled_at ? a.id.localeCompare(b.id) : Date.parse(a.scheduled_at) - Date.parse(b.scheduled_at)

export const createScheduledStore = () =>
  createStore<ScheduledStoreState>()((set) => ({
    byChat: {},
    replace: (chatId, items) => set((state) => ({ byChat: { ...state.byChat, [chatId]: [...items].sort(byTime) } })),
    upsert: (item) =>
      set((state) => {
        const current = (state.byChat[item.chat_id] ?? []).filter((existing) => existing.id !== item.id)
        return { byChat: { ...state.byChat, [item.chat_id]: [...current, item].sort(byTime) } }
      }),
    remove: (chatId, id) =>
      set((state) => {
        const current = state.byChat[chatId]
        if (!current?.some((item) => item.id === id)) return state
        return { byChat: { ...state.byChat, [chatId]: current.filter((item) => item.id !== id) } }
      }),
    dropDelivered: (chatId, messageIds) =>
      set((state) => {
        const current = state.byChat[chatId]
        if (!current?.some((item) => messageIds.has(item.id))) return state
        return { byChat: { ...state.byChat, [chatId]: current.filter((item) => !messageIds.has(item.id)) } }
      }),
    clear: () => set({ byChat: {} }),
  }))

export type ScheduledStore = ReturnType<typeof createScheduledStore>

export const scheduledStore = createScheduledStore()

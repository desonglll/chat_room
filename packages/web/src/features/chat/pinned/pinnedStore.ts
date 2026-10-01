/**
 * TG-901: pinned messages per chat, refreshed when a chat opens, when its socket reports
 * `pins_changed`, and after the viewer's own pin/unpin. Framework-free.
 */
import { createStore } from 'zustand/vanilla'
import { authStore, selectToken } from '@tg/core'
import { apiClient } from '../../../app/client'
import { listPins } from './pinnedApi'
import type { PinEntry } from './pinnedModel'
import { toPinEntries } from './pinnedModel'

export interface PinnedState {
  byChat: Record<string, PinEntry[]>
}

export const pinnedStore = createStore<PinnedState>()(() => ({ byChat: {} }))

const EMPTY: PinEntry[] = []
export const selectPins = (chatId: string) => (state: PinnedState) => state.byChat[chatId] ?? EMPTY

/** Re-read `chatId`'s pins; a failed read keeps what the bar already shows. */
export async function refreshPins(chatId: string): Promise<void> {
  try {
    const pins = await listPins(apiClient, selectToken(authStore.getState()), chatId)
    pinnedStore.setState((state) => ({ byChat: { ...state.byChat, [chatId]: toPinEntries(pins) } }))
  } catch {
    // Offline or no longer a member: leave the bar as it was.
  }
}

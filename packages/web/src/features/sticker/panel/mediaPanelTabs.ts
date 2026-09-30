/**
 * Extension point for the media panel's top tabs. The panel owns 表情 and 贴纸; every other
 * tab registers here. `gif` has a built-in placeholder until TG-305 registers the real one:
 *
 *     registerMediaPanelTab({ id: 'gif', label: 'GIF', order: 30, render: (ctx) => <GifTab {...ctx} /> })
 *
 * Registration replaces an earlier tab with the same id and returns an undo.
 */
import type { ReactNode } from 'react'
import { createStore } from 'zustand/vanilla'

export interface MediaPanelTabContext {
  chatId: string
  canSend: boolean
  /** Close the panel, e.g. after sending. */
  close(): void
}

export interface MediaPanelTab {
  id: string
  label: string
  /** Built-ins: emoji 10, stickers 20, gif 30. */
  order: number
  render(context: MediaPanelTabContext): ReactNode
}

export const mediaPanelTabStore = createStore<{ tabs: readonly MediaPanelTab[] }>()(() => ({ tabs: [] }))

export function registerMediaPanelTab(tab: MediaPanelTab): () => void {
  const previous = mediaPanelTabStore.getState().tabs.find((entry) => entry.id === tab.id)
  const others = () => mediaPanelTabStore.getState().tabs.filter((entry) => entry.id !== tab.id)
  mediaPanelTabStore.setState({ tabs: [...others(), tab].sort((a, b) => a.order - b.order) })
  return () => {
    if (!mediaPanelTabStore.getState().tabs.includes(tab)) return
    const restored = previous ? [...others(), previous] : others()
    mediaPanelTabStore.setState({ tabs: restored.sort((a, b) => a.order - b.order) })
  }
}

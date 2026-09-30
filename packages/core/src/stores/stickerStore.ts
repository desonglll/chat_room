/**
 * Sticker packs, recents and favorites. Pure skeleton: M3 (TG-30x) defines the sticker
 * set wire types when the `sticker_sets` tables exist; until then the shapes here are the
 * minimal client-side placeholders the panel skeleton needs.
 */
import { createStore } from 'zustand/vanilla'

export interface StickerRef {
  set_id: string
  sticker_id: string
  emoji: string
}

export interface StickerSetSummary {
  id: string
  title: string
  count: number
}

export interface StickerState {
  sets: StickerSetSummary[]
  recent: StickerRef[]
  favorites: StickerRef[]
  setSets(sets: StickerSetSummary[]): void
  pushRecent(sticker: StickerRef, limit?: number): void
  toggleFavorite(sticker: StickerRef): void
}

const sameSticker = (a: StickerRef, b: StickerRef) => a.set_id === b.set_id && a.sticker_id === b.sticker_id

export const createStickerStore = () =>
  createStore<StickerState>()((set) => ({
    sets: [],
    recent: [],
    favorites: [],
    setSets: (sets) => set({ sets }),
    pushRecent: (sticker, limit = 32) =>
      set((state) => ({
        recent: [sticker, ...state.recent.filter((existing) => !sameSticker(existing, sticker))].slice(0, limit),
      })),
    toggleFavorite: (sticker) =>
      set((state) => ({
        favorites: state.favorites.some((existing) => sameSticker(existing, sticker))
          ? state.favorites.filter((existing) => !sameSticker(existing, sticker))
          : [...state.favorites, sticker],
      })),
  }))

export type StickerStore = ReturnType<typeof createStickerStore>

export const stickerStore = createStickerStore()

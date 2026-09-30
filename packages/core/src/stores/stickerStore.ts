/**
 * The viewer's sticker library (TG-303): installed sets (server order, archived included),
 * recents and favorites. The server is the source of truth — every library write answers
 * the whole `InstalledStickerSets` with a `revision`, and `applyInstalled` drops a response
 * older than the one held, so two overlapping writes can never roll the panel back.
 *
 * `pushRecent` / `setFavorite` are the optimistic local halves of a send and a favorite
 * toggle; they mirror the server's caps (20 recents, 5 favorites, oldest favorite evicted).
 *
 * Emoji suggestions read `suggestStickers`, a lookup over an emoji → stickers index built
 * once per `sets` identity, so a keystroke costs a Map lookup, never a scan or a request.
 */
import { createStore } from 'zustand/vanilla'
import type { InstalledStickerSets, Sticker, StickerSet } from '../api/stickers'

export const RECENT_STICKER_LIMIT = 20
export const FAVORITE_STICKER_LIMIT = 5

export type StickerLibraryStatus = 'idle' | 'loading' | 'ready' | 'error'

export interface StickerState {
  status: StickerLibraryStatus
  /** Last applied library revision; `-1` before the first load. */
  revision: number
  sets: StickerSet[]
  recent: Sticker[]
  favorites: Sticker[]
  setStatus(status: StickerLibraryStatus): void
  /** Replace the installed sets unless `result` is older than what is held. */
  applyInstalled(result: InstalledStickerSets): void
  setRecent(stickers: Sticker[]): void
  setFavorites(stickers: Sticker[]): void
  /** A sticker was just sent: move it to the front of the recents. */
  pushRecent(sticker: Sticker, limit?: number): void
  removeRecent(stickerId: string): void
  setFavorite(sticker: Sticker, favorite: boolean, limit?: number): void
  reset(): void
}

const without = (list: readonly Sticker[], id: string) => list.filter((sticker) => sticker.id !== id)

const INITIAL = { status: 'idle' as StickerLibraryStatus, revision: -1, sets: [], recent: [], favorites: [] }

export const createStickerStore = () =>
  createStore<StickerState>()((set) => ({
    ...INITIAL,
    setStatus: (status) => set({ status }),
    applyInstalled: (result) =>
      set((state) => (result.revision < state.revision ? {} : { revision: result.revision, sets: result.sets })),
    setRecent: (recent) => set({ recent: recent.slice(0, RECENT_STICKER_LIMIT) }),
    setFavorites: (favorites) => set({ favorites: favorites.slice(0, FAVORITE_STICKER_LIMIT) }),
    pushRecent: (sticker, limit = RECENT_STICKER_LIMIT) =>
      set((state) => ({ recent: [sticker, ...without(state.recent, sticker.id)].slice(0, limit) })),
    removeRecent: (stickerId) => set((state) => ({ recent: without(state.recent, stickerId) })),
    setFavorite: (sticker, favorite, limit = FAVORITE_STICKER_LIMIT) =>
      set((state) => ({
        favorites: favorite
          ? [sticker, ...without(state.favorites, sticker.id)].slice(0, limit)
          : without(state.favorites, sticker.id),
      })),
    reset: () => set(INITIAL),
  }))

export type StickerStore = ReturnType<typeof createStickerStore>

export const stickerStore = createStickerStore()

/** Installed, non-archived regular sets in the viewer's order — what the panel shows. */
export function selectActiveStickerSets(state: Pick<StickerState, 'sets'>): StickerSet[] {
  return activeCache(state.sets)
}

export function selectArchivedStickerSets(state: Pick<StickerState, 'sets'>): StickerSet[] {
  return state.sets.filter((set) => set.archived)
}

export const selectIsFavoriteSticker = (stickerId: string) => (state: Pick<StickerState, 'favorites'>) =>
  state.favorites.some((sticker) => sticker.id === stickerId)

/**
 * Emoji identity for matching: variation selectors (U+FE0E/U+FE0F) are dropped, because
 * keyboards and servers disagree on whether "❤" carries one; skin tones are kept, because
 * "👍🏽" is a different request from "👍".
 */
export function normalizeStickerEmoji(emoji: string): string {
  return emoji.replace(/[\uFE0E\uFE0F]/g, '').trim()
}

/**
 * Stickers answering to `emoji`: favorites first, then recents, then installed sets in the
 * viewer's order, de-duplicated, at most `limit`. Same visibility as `GET /api/stickers/search`
 * (non-archived installed sets), plus the viewer's own favorites and recents.
 */
export function suggestStickers(
  state: Pick<StickerState, 'sets' | 'recent' | 'favorites'>,
  emoji: string,
  limit = 20,
): Sticker[] {
  const key = normalizeStickerEmoji(emoji)
  if (!key) return []
  const answers = (sticker: Sticker) => stickerEmojiKeys(sticker).includes(key)
  const picked: Sticker[] = []
  const seen = new Set<string>()
  const take = (sticker: Sticker) => {
    if (picked.length >= limit || seen.has(sticker.id)) return
    seen.add(sticker.id)
    picked.push(sticker)
  }
  for (const sticker of state.favorites) if (answers(sticker)) take(sticker)
  for (const sticker of state.recent) if (answers(sticker)) take(sticker)
  for (const sticker of indexCache(state.sets).get(key) ?? []) take(sticker)
  return picked
}

function stickerEmojiKeys(sticker: Sticker): string[] {
  const all = sticker.emojis.length > 0 ? sticker.emojis : [sticker.emoji]
  return all.map(normalizeStickerEmoji)
}

/** One-entry memo per derived view, keyed by the `sets` array identity (store writes replace it). */
function memoBySets<T>(derive: (sets: StickerSet[]) => T): (sets: StickerSet[]) => T {
  let lastSets: StickerSet[] | null = null
  let lastValue: T
  return (sets) => {
    if (sets !== lastSets) {
      lastSets = sets
      lastValue = derive(sets)
    }
    return lastValue
  }
}

const activeCache = memoBySets((sets) => sets.filter((set) => !set.archived && set.set_type === 'regular'))

const indexCache = memoBySets((sets) => {
  const index = new Map<string, Sticker[]>()
  for (const set of activeCache(sets)) {
    for (const sticker of set.stickers) {
      for (const key of new Set(stickerEmojiKeys(sticker))) {
        const bucket = index.get(key)
        if (bucket) bucket.push(sticker)
        else index.set(key, [sticker])
      }
    }
  }
  return index
})

/**
 * The list of one Chat's media the viewer walks through, and how it grows.
 *
 * Source of truth is the server's `/api/chats/:id/files` listing (newest first, keyset cursor
 * = a message id, no "after" direction). The viewer opens instantly on the `seed` — the media
 * of the already-loaded timeline — and then:
 *
 *  1. `start()` syncs the newest end: it pages from the head until the pages reach the seed
 *     (or find the target when there is no seed), so media newer than the loaded window is
 *     not silently skipped. Bounded by `maxSyncPages`; past that the seed alone is kept.
 *  2. `loadOlder()` extends the oldest end, one page per call.
 *
 * The listing has no "images and videos" kind, so pages are requested as `all` and filtered
 * by the adapter; a page may therefore add zero items and still advance the cursor.
 * Framework-free: tests drive it with a fake `fetchPage`.
 */
import type { MediaItem } from './mediaItem'
import { mergeItems } from './mediaItem'

export interface MediaPage {
  /** The page's viewable media (already filtered to photos/videos). */
  items: MediaItem[]
  /** The server's `next_before`: the page's oldest row id, or null at the start of the chat. */
  nextBefore: string | null
  /** `created_at` of the page's oldest row of ANY kind, or null for an empty page. */
  oldestAt: string | null
}

export type FetchMediaPage = (before: string | null) => Promise<MediaPage>

export interface PagerState {
  items: MediaItem[]
  hasOlder: boolean
  loadingOlder: boolean
  syncing: boolean
  /** The last request failed; the list is still usable, a later `loadOlder()` retries. */
  failed: boolean
}

export interface MediaPager {
  getState(): PagerState
  subscribe(listener: (state: PagerState) => void): () => void
  start(): Promise<void>
  loadOlder(): Promise<void>
  /** Drop an item (deleted from the viewer); it never comes back from a later page. */
  remove(attachmentId: string): void
}

export interface MediaPagerOptions {
  seed: MediaItem[]
  targetId: string
  fetchPage: FetchMediaPage
  maxSyncPages?: number
}

const time = (iso: string): number => Date.parse(iso)

export function createMediaPager({ seed, targetId, fetchPage, maxSyncPages = 8 }: MediaPagerOptions): MediaPager {
  const listeners = new Set<(state: PagerState) => void>()
  const removed = new Set<string>()
  let started = false
  let olderCursor: string | null = seed[0]?.messageId ?? null
  let state: PagerState = { items: seed, hasOlder: true, loadingOlder: false, syncing: false, failed: false }

  const set = (patch: Partial<PagerState>) => {
    const items = patch.items ? patch.items.filter((item) => !removed.has(item.attachmentId)) : state.items
    state = { ...state, ...patch, items }
    for (const listener of listeners) listener(state)
  }

  const seedOldest = seed[0]
  const seedNewest = seed.at(-1)
  const seedHasTarget = seed.some((item) => item.attachmentId === targetId)
  const reachesSeed = (page: MediaPage) =>
    seedNewest === undefined || (page.oldestAt !== null && time(page.oldestAt) <= time(seedNewest.createdAt))

  /** Idempotent: a second call (React StrictMode's double effect) does nothing. */
  async function start(): Promise<void> {
    if (started) return
    started = true
    set({ syncing: true })
    let fetched: MediaItem[] = []
    let last: MediaPage | null = null
    let cursor: string | null = null
    try {
      for (let pages = 0; pages < maxSyncPages; pages += 1) {
        const page = await fetchPage(cursor)
        fetched = mergeItems(fetched, page.items)
        last = page
        const found = seedHasTarget || fetched.some((item) => item.attachmentId === targetId)
        if ((found && reachesSeed(page)) || page.nextBefore === null) break
        cursor = page.nextBefore
      }
    } catch {
      set({ syncing: false, failed: true })
      return
    }
    if (last === null) return set({ syncing: false })

    const contiguous = reachesSeed(last) || last.nextBefore === null
    if (contiguous) {
      const seedIsOlder =
        seedOldest !== undefined && last.oldestAt !== null && time(seedOldest.createdAt) < time(last.oldestAt)
      olderCursor = last.nextBefore === null ? null : seedIsOlder ? seedOldest.messageId : last.nextBefore
      return set({ items: mergeItems(state.items, fetched), hasOlder: olderCursor !== null, syncing: false })
    }
    if (!seedHasTarget && fetched.some((item) => item.attachmentId === targetId)) {
      // The seed is a window far from the head that does not even hold the target: use the
      // fetched range alone, which is contiguous from the head.
      olderCursor = last.nextBefore
      return set({ items: fetched, hasOlder: true, syncing: false })
    }
    // Gap between the head and the seed (a deep-linked window): keep the seed only.
    set({ syncing: false })
  }

  async function loadOlder(): Promise<void> {
    if (!state.hasOlder || state.loadingOlder || state.syncing || olderCursor === null) return
    set({ loadingOlder: true, failed: false })
    try {
      const page = await fetchPage(olderCursor)
      olderCursor = page.nextBefore
      set({ items: mergeItems(state.items, page.items), hasOlder: page.nextBefore !== null, loadingOlder: false })
    } catch {
      set({ loadingOlder: false, failed: true })
    }
  }

  return {
    getState: () => state,
    subscribe(listener) {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    start,
    loadOlder,
    remove(attachmentId) {
      removed.add(attachmentId)
      set({ items: state.items })
    },
  }
}

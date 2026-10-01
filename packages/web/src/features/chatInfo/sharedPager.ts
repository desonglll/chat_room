/**
 * One pagination cursor for one shared-content tab. Every tab (media, files, links,
 * voice, GIF, members) owns its own pager, so loading page 3 of "files" never moves the
 * "links" cursor — the acceptance criterion "各自独立分页".
 *
 * A vanilla zustand store (same library the rest of the client uses) so a tab component
 * subscribes to exactly its own pager and tests drive it without React.
 */
import { createStore } from 'zustand/vanilla'

export interface SharedPage<T> {
  items: T[]
  /** Opaque cursor for the next (older) page; `null` = this was the last page. */
  next: string | null
}

/** `cursor` is `null` for the first page. */
export type FetchSharedPage<T> = (cursor: string | null) => Promise<SharedPage<T>>

export interface SharedPagerState<T> {
  items: T[]
  cursor: string | null
  /** At least one page arrived (distinguishes "empty" from "not asked yet"). */
  started: boolean
  done: boolean
  loading: boolean
  error: string
  /** Fetch the next page; a no-op while loading or when done. Resolves when it settles. */
  loadMore(): Promise<void>
}

export type SharedPager<T> = ReturnType<typeof createSharedPager<T>>

export function createSharedPager<T>(fetchPage: FetchSharedPage<T>, keyOf: (item: T) => string) {
  const store = createStore<SharedPagerState<T>>()((set, get) => ({
    items: [],
    cursor: null,
    started: false,
    done: false,
    loading: false,
    error: '',
    loadMore: async () => {
      const state = get()
      if (state.loading || state.done) return
      set({ loading: true, error: '' })
      try {
        const page = await fetchPage(state.cursor)
        const seen = new Set(get().items.map(keyOf))
        const fresh = page.items.filter((item) => {
          const key = keyOf(item)
          if (seen.has(key)) return false
          seen.add(key)
          return true
        })
        set((current) => ({
          items: fresh.length > 0 ? [...current.items, ...fresh] : current.items,
          cursor: page.next,
          done: page.next === null,
          started: true,
          loading: false,
        }))
      } catch (error) {
        set({ loading: false, started: true, error: error instanceof Error ? error.message : String(error) })
      }
    },
  }))
  return store
}

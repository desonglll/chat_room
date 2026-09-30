/**
 * The virtual list's loaded-window state machine (TG-101), framework-free so
 * `messageListController.test.ts` drives paging, jumps, joins and returns against fake
 * APIs. The view owns only DOM concerns (Virtuoso, measuring the return anchor).
 *
 * Two modes:
 * - `live`: rows are the store timeline, with older REST pages prepended INTO the store
 *   (`prependLive` → `messageStore.prependHistory`, TG-100), so edits/recalls/reactions/
 *   optimistic sends from the chat session reach every loaded row. Without `prependLive`
 *   (unit tests) pages go to the private `older` array in front of the timeline instead.
 * - `detached`: a jump landed outside the loaded range; rows are one contiguous `detached`
 *   window fetched around the target. Paging newer grows it until it overlaps the live
 *   timeline, at which point it JOINS back into `live` without remounting the list.
 *
 * Invariant for Virtuoso: `firstItemIndex` drops by exactly the number of rows inserted in
 * front of the previous first row; a wholesale replacement bumps `viewKey` instead (remount).
 * When more history exists, the oldest loaded message is kept hidden (the "lead") so the
 * first visible row's grouping/date header is already final and a prepend never changes it.
 */
import { createStore } from 'zustand/vanilla'
import type { BroadcastMessage } from '@tg/core'
import {
  broadcastIds,
  mergeMessagePages,
  messageKey,
  newestCursor,
  oldestCursor,
  overlapsLive,
  prependShift,
  splitContextWindow,
  withoutLive,
} from '@tg/core'
import type { MessageListControllerOptions, MessageListState, ReturnAnchor, ScrollLocation } from './messageListState'
import {
  CONTEXT_WINDOW_SIZE,
  FIRST_ITEM_INDEX_BASE,
  HIGHLIGHT_MS,
  LOAD_FAILED_NOTICE,
  MESSAGE_NOT_FOUND_NOTICE,
  OLDER_PAGE_SIZE,
  initialMessageListState,
  rowIndexOf,
  visibleKeys,
  visibleWindow,
} from './messageListState'

export * from './messageListState'

export function createMessageListController(options: MessageListControllerOptions) {
  const store = createStore<MessageListState>()(() => initialMessageListState())
  const { api, getLive, timers } = options
  let disposed = false
  let highlightTimer: unknown = null
  let scrollSeq = 0

  const get = () => store.getState()
  const set = (patch: Partial<MessageListState>) => {
    if (!disposed) store.setState(patch)
  }
  const current = () => visibleWindow(get(), getLive())

  /** Apply a patch that may prepend rows, keeping Virtuoso's anchor: shift firstItemIndex. */
  function setPreservingAnchor(patch: Partial<MessageListState>, before = current()): void {
    const previousFirst = before.all[before.hidden]
    const next = { ...get(), ...patch }
    const shift = previousFirst
      ? prependShift(messageKey(previousFirst), visibleKeys(visibleWindow(next, getLive())))
      : 0
    if (shift > 0) options.beforePrepend?.()
    set({ ...patch, firstItemIndex: get().firstItemIndex - shift })
  }

  function requestScroll(location: ScrollLocation, smooth = false): void {
    scrollSeq += 1
    set({ scrollRequest: { ...location, seq: scrollSeq, smooth } })
  }

  function highlight(messageId: string): void {
    if (highlightTimer !== null) timers.clearTimeout(highlightTimer)
    set({ highlightedId: messageId })
    highlightTimer = timers.setTimeout(() => {
      highlightTimer = null
      set({ highlightedId: '' })
    }, HIGHLIGHT_MS)
  }

  async function loadOlder(): Promise<void> {
    const state = get()
    const detached = state.mode === 'detached'
    const hasMore = detached ? state.detachedHasOlder : state.olderHasMore
    if (state.loadingOlder || !hasMore) return
    const loaded = current()
    const cursor = oldestCursor(loaded.all)
    if (!cursor) {
      // Nothing server-side loaded yet (empty chat or only pending rows): nothing older.
      if (!detached && loaded.all.length > 0) set({ olderHasMore: false })
      return
    }
    set({ loadingOlder: true })
    let page: BroadcastMessage[]
    try {
      ;[page] = await Promise.all([api.loadOlder(cursor, OLDER_PAGE_SIZE), options.afterPaint?.()])
    } catch {
      set({ loadingOlder: false, notice: LOAD_FAILED_NOTICE })
      return
    }
    if (disposed) return
    if (get().mode !== state.mode || get().viewKey !== state.viewKey) return set({ loadingOlder: false })
    const more = page.length >= OLDER_PAGE_SIZE
    if (detached) {
      const merged = mergeMessagePages(get().detached, page)
      setPreservingAnchor({
        detached: merged.messages,
        detachedHasOlder: more && merged.added > 0,
        loadingOlder: false,
      })
    } else if (options.prependLive) {
      const before = current()
      const added = options.prependLive(page)
      setPreservingAnchor({ olderHasMore: more && added > 0, loadingOlder: false }, before)
    } else {
      const merged = mergeMessagePages(get().older, withoutLive(page, broadcastIds(getLive())))
      setPreservingAnchor({ older: merged.messages, olderHasMore: more && merged.added > 0, loadingOlder: false })
    }
  }

  /**
   * Fold a contiguous range that overlaps the live rows into the live window. Writes the
   * store when `prependLive` is set: callers capture `current()` BEFORE calling this.
   */
  function joinIntoLive(range: BroadcastMessage[], rangeReachedStart: boolean): Partial<MessageListState> {
    const state = get()
    if (options.prependLive) {
      const added = options.prependLive(range)
      return {
        mode: 'live',
        olderHasMore: added > 0 ? !rangeReachedStart : state.olderHasMore,
        detached: [],
        detachedHasOlder: false,
        detachedHasNewer: false,
      }
    }
    const liveIds = broadcastIds(getLive())
    const merged = mergeMessagePages(state.older, withoutLive(range, liveIds))
    const rangeFirst = range[0]
    const extendsBack =
      rangeFirst !== undefined && merged.messages[0]?.message_id === rangeFirst.message_id && merged.added > 0
    return {
      mode: 'live',
      older: merged.messages,
      olderHasMore: extendsBack ? !rangeReachedStart : state.olderHasMore,
      detached: [],
      detachedHasOlder: false,
      detachedHasNewer: false,
    }
  }

  async function loadNewer(): Promise<void> {
    const state = get()
    if (state.mode !== 'detached' || state.loadingNewer || !state.detachedHasNewer) return
    const cursor = newestCursor(state.detached)
    if (!cursor) return
    set({ loadingNewer: true })
    let context: BroadcastMessage[]
    try {
      context = await api.loadAround(cursor, CONTEXT_WINDOW_SIZE)
    } catch {
      set({ loadingNewer: false, notice: LOAD_FAILED_NOTICE })
      return
    }
    if (disposed) return
    if (get().mode !== 'detached' || get().viewKey !== state.viewKey) return set({ loadingNewer: false })
    const { reachedEnd } = splitContextWindow(context, cursor, CONTEXT_WINDOW_SIZE)
    const merged = mergeMessagePages(get().detached, context)
    if (reachedEnd || overlapsLive(merged.messages, broadcastIds(getLive()))) {
      const before = current()
      setPreservingAnchor({ ...joinIntoLive(merged.messages, !get().detachedHasOlder), loadingNewer: false }, before)
      return
    }
    set({ detached: merged.messages, detachedHasNewer: merged.added > 0, loadingNewer: false })
  }

  /**
   * Scroll to a message, loading a window around it when it is not loaded. `anchor` is the
   * position to come back to (null keeps any existing one, e.g. for a deep link).
   */
  async function jumpTo(messageId: string, anchor: ReturnAnchor | null): Promise<void> {
    if (!messageId) return
    if (anchor) set({ returnAnchor: anchor, notice: '' })
    const loaded = rowIndexOf(current(), messageId)
    if (loaded >= 0) {
      requestScroll({ index: loaded, align: 'center' }, true)
      highlight(messageId)
      return
    }
    const viewKey = get().viewKey
    set({ jumping: true })
    let context: BroadcastMessage[]
    try {
      context = await api.loadAround(messageId, CONTEXT_WINDOW_SIZE)
    } catch {
      set({ jumping: false, notice: LOAD_FAILED_NOTICE })
      return
    }
    if (disposed) return
    if (get().viewKey !== viewKey) return set({ jumping: false })
    const { targetIndex, reachedStart, reachedEnd } = splitContextWindow(context, messageId, CONTEXT_WINDOW_SIZE)
    if (targetIndex < 0) {
      set({ jumping: false, notice: MESSAGE_NOT_FOUND_NOTICE })
      return
    }
    const state = get()
    const liveIds = broadcastIds(getLive())
    const joinsLive = overlapsLive(context, liveIds) || overlapsLive(context, broadcastIds(state.older)) || reachedEnd
    if (state.mode === 'live' && joinsLive) {
      const before = current()
      setPreservingAnchor({ ...joinIntoLive(context, reachedStart), jumping: false }, before)
    } else if (state.mode === 'detached' && overlapsLive(context, broadcastIds(state.detached))) {
      const merged = mergeMessagePages(state.detached, context)
      const extendsBack = merged.messages[0]?.message_id === context[0]?.message_id && merged.added > 0
      setPreservingAnchor({
        detached: merged.messages,
        detachedHasOlder: extendsBack ? !reachedStart : state.detachedHasOlder,
        jumping: false,
      })
    } else {
      const next: MessageListState = {
        ...state,
        mode: 'detached',
        detached: context,
        detachedHasOlder: !reachedStart,
        detachedHasNewer: !reachedEnd,
        detachedLiveMark: state.mode === 'detached' ? state.detachedLiveMark : getLive().length,
      }
      const index = rowIndexOf(visibleWindow(next, getLive()), messageId)
      set({
        ...next,
        jumping: false,
        loadingOlder: false,
        loadingNewer: false,
        viewKey: state.viewKey + 1,
        firstItemIndex: FIRST_ITEM_INDEX_BASE,
        initialLocation: { index: Math.max(0, index), align: 'center' },
        scrollRequest: null,
      })
      highlight(messageId)
      return
    }
    const index = rowIndexOf(current(), messageId)
    if (index >= 0) requestScroll({ index, align: 'center' }, false)
    highlight(messageId)
  }

  /** Leave a detached window (if any) and show the newest message. */
  function goToLatest(): void {
    const state = get()
    set({ returnAnchor: null })
    if (state.mode === 'detached') {
      set({
        mode: 'live',
        detached: [],
        detachedHasOlder: false,
        detachedHasNewer: false,
        loadingOlder: false,
        loadingNewer: false,
        viewKey: state.viewKey + 1,
        firstItemIndex: FIRST_ITEM_INDEX_BASE,
        initialLocation: null,
        scrollRequest: null,
      })
      return
    }
    requestScroll({ index: 'LAST', align: 'end' }, true)
  }

  /** "Back to where you were": restore the anchor captured before the last jump. */
  async function returnToAnchor(): Promise<void> {
    const anchor = get().returnAnchor
    if (!anchor) return
    if (anchor.atBottom) {
      goToLatest()
      return
    }
    set({ returnAnchor: null })
    const state = get()
    const location = (index: number): ScrollLocation => ({ index, align: 'start', offset: -anchor.offsetPx })
    if (state.mode === 'live') {
      const index = rowIndexOf(current(), anchor.key)
      if (index >= 0) {
        requestScroll(location(index))
        return
      }
    } else {
      const live: MessageListState = { ...state, mode: 'live', detached: [] }
      const index = rowIndexOf(visibleWindow(live, getLive()), anchor.key)
      if (index >= 0) {
        set({
          ...live,
          detachedHasOlder: false,
          detachedHasNewer: false,
          loadingOlder: false,
          loadingNewer: false,
          viewKey: state.viewKey + 1,
          firstItemIndex: FIRST_ITEM_INDEX_BASE,
          initialLocation: location(index),
          scrollRequest: null,
        })
        return
      }
    }
    await jumpTo(anchor.key, null)
  }

  return {
    store,
    loadOlder,
    loadNewer,
    jumpTo,
    goToLatest,
    returnToAnchor,
    dismissNotice: () => set({ notice: '' }),
    /**
     * Stop applying late responses. Reversible via `resume()` because React StrictMode
     * (dev) unmounts and remounts effects on the SAME memoized controller.
     */
    dispose() {
      disposed = true
      if (highlightTimer !== null) timers.clearTimeout(highlightTimer)
      highlightTimer = null
      if (get().highlightedId) store.setState({ highlightedId: '' })
    },
    resume() {
      disposed = false
    },
  }
}

export type MessageListController = ReturnType<typeof createMessageListController>

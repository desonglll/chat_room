/**
 * React binding for the list controller: one controller per mounted chat list, its store
 * subscribed, the visible window and the incremental layout memoized. Also the one DOM
 * helper the controller needs from the view — capturing the return anchor.
 */
import { useEffect, useMemo, useRef } from 'react'
import type { DisplayMessage, MessageLayoutEntry } from '@tg/core'
import { authStore, createMessageLayoutCache, messageStore, selectTimeline, selectToken } from '@tg/core'
import { useStore } from 'zustand/react'
import { apiClient } from '../../app/client'
import { createMessageListApi } from './messageListApi'
import type { MessageListApi, ReturnAnchor } from './messageListController'
import { createMessageListController, visibleWindow } from './messageListController'

export interface UseMessageListControllerInput {
  chatId: string
  currentUserId: string
  showGroupIdentity: boolean
  live: DisplayMessage[]
  api: MessageListApi | undefined
  /** Called right before rows are prepended (the view pins its scroll anchor). */
  onBeforePrepend: () => void
}

const browserTimers = {
  setTimeout: (handler: () => void, ms: number): unknown => globalThis.setTimeout(handler, ms),
  clearTimeout: (handle: unknown) => globalThis.clearTimeout(handle as ReturnType<typeof setTimeout>),
}

export function useMessageListController({
  chatId,
  currentUserId,
  showGroupIdentity,
  live,
  api,
  onBeforePrepend,
}: UseMessageListControllerInput) {
  const beforePrependRef = useRef(onBeforePrepend)
  beforePrependRef.current = onBeforePrepend
  const controller = useMemo(
    () =>
      createMessageListController({
        api: api ?? createMessageListApi(apiClient, chatId, () => selectToken(authStore.getState())),
        // The store, not the last rendered prop: `prependLive` writes it and the anchor
        // math reads the result in the same tick, before React re-renders.
        getLive: () => selectTimeline(chatId)(messageStore.getState()).messages,
        prependLive: (rows) => messageStore.getState().prependHistory(chatId, rows),
        timers: browserTimers,
        beforePrepend: () => beforePrependRef.current(),
        afterPaint: () =>
          new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => setTimeout(resolve, 0)))),
      }),
    [chatId, api],
  )
  useEffect(() => {
    controller.resume()
    return () => controller.dispose()
  }, [controller])
  const state = useStore(controller.store)
  // Only the fields that shape the rows: a highlight or scroll request must not rebuild them.
  const { mode, older, olderHasMore, detached, detachedHasOlder } = state
  const view = useMemo(
    () => visibleWindow({ mode, older, olderHasMore, detached, detachedHasOlder }, live),
    [mode, older, olderHasMore, detached, detachedHasOlder, live],
  )
  const layoutCache = useMemo(() => createMessageLayoutCache(), [controller])
  const entries: MessageLayoutEntry[] = useMemo(
    () => layoutCache(view.all, { currentUserId, showGroupIdentity }),
    [layoutCache, view.all, currentUserId, showGroupIdentity],
  )
  return { controller, state, view, entries }
}

/** The topmost visible row and its offset from the viewport top, for "back to where you were". */
export function captureAnchor(scroller: HTMLElement | null, atBottom: boolean): ReturnAnchor | null {
  if (!scroller) return null
  const top = scroller.getBoundingClientRect().top
  for (const row of scroller.querySelectorAll<HTMLElement>('[data-row-key]')) {
    const rect = row.getBoundingClientRect()
    if (rect.bottom > top + 1) return { key: row.dataset.rowKey ?? '', offsetPx: rect.top - top, atBottom }
  }
  return null
}

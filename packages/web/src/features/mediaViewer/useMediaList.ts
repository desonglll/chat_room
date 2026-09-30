/**
 * The viewer session's media list and cursor: a pager seeded from the Chat's loaded
 * timeline, the current attachment id, and the older-page trigger.
 */
import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from 'react'
import { messageStore } from '@tg/core'
import type { MediaItem } from './mediaItem'
import { mediaFromMessages } from './mediaItem'
import type { FetchMediaPage, PagerState } from './mediaPager'
import { createMediaPager } from './mediaPager'
import type { Direction } from './mediaNavigation'
import { indexOfItem, neighbourId, shouldLoadOlder, successorAfterRemoval } from './mediaNavigation'

export interface MediaList extends PagerState {
  current: MediaItem | null
  index: number
  go(direction: Direction): boolean
  select(attachmentId: string): void
  /** Remove the current item; returns the id now shown, or null when the list is empty. */
  removeCurrent(): string | null
  neighbour(direction: Direction): MediaItem | null
}

export function useMediaList(chatId: string, targetId: string, fetchPage: FetchMediaPage): MediaList {
  const [pager] = useState(() =>
    createMediaPager({
      seed: mediaFromMessages(messageStore.getState().timelines[chatId]?.messages ?? []),
      targetId,
      fetchPage,
    }),
  )
  const state = useSyncExternalStore(pager.subscribe, pager.getState, pager.getState)
  const [currentId, setCurrentState] = useState(targetId)
  // Read by `go`: two key presses can land before React re-renders with the first result.
  const currentRef = useRef(targetId)
  const setCurrentId = useCallback((id: string) => {
    currentRef.current = id
    setCurrentState(id)
  }, [])

  useEffect(() => {
    void pager.start()
  }, [pager])

  const index = indexOfItem(state.items, currentId)
  useEffect(() => {
    if (shouldLoadOlder(index, state.hasOlder)) void pager.loadOlder()
  }, [pager, index, state.hasOlder, state.syncing])

  const go = useCallback(
    (direction: Direction) => {
      const next = neighbourId(pager.getState().items, currentRef.current, direction)
      if (next) setCurrentId(next)
      return next !== null
    },
    [pager, setCurrentId],
  )

  const removeCurrent = useCallback(() => {
    const removed = currentRef.current
    const next = successorAfterRemoval(pager.getState().items, removed)
    pager.remove(removed)
    if (next) setCurrentId(next)
    return next
  }, [pager, setCurrentId])

  const neighbour = (direction: Direction) => state.items[index + direction] ?? null

  return {
    ...state,
    current: state.items[index] ?? null,
    index,
    go,
    select: setCurrentId,
    removeCurrent,
    neighbour: (direction) => (index < 0 ? null : neighbour(direction)),
  }
}

/**
 * Friends' presence for the contacts list. TG-1102: the account socket pushes
 * `friend_statuses` whenever a friend's presence changes (same privacy rules as the REST read),
 * so the list is live; the REST read covers the first paint and any time no push has arrived
 * yet. A failed read keeps the last answer.
 */
import { useEffect, useState } from 'react'
import { useStore } from 'zustand/react'
import type { SocialApi, UserStatusEntry } from '@tg/core'
import { notificationsStore } from '../notifications/notificationsStore'

export function useFriendStatuses(api: SocialApi, friendCount: number): UserStatusEntry[] {
  const [fetched, setFetched] = useState<UserStatusEntry[]>([])
  const pushed = useStore(notificationsStore, (state) => state.friendStatuses)
  useEffect(() => {
    if (typeof api.friendStatuses !== 'function') return
    let alive = true
    api.friendStatuses().then(
      (next) => {
        if (alive) setFetched(next)
      },
      () => undefined,
    )
    return () => {
      alive = false
    }
  }, [api, friendCount])
  return pushed ?? fetched
}

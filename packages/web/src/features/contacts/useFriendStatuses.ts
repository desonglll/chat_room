/**
 * TG-903: friends' presence for the contacts list — read on load, whenever the friend count
 * changes, and every 60 s (Telegram's list is live; a cheap poll keeps «N 分钟前» honest
 * without a per-friend socket subscription). A failed read keeps the last answer.
 */
import { useEffect, useState } from 'react'
import type { SocialApi, UserStatusEntry } from '@tg/core'

const REFRESH_MS = 60_000

export function useFriendStatuses(api: SocialApi, friendCount: number): UserStatusEntry[] {
  const [statuses, setStatuses] = useState<UserStatusEntry[]>([])
  useEffect(() => {
    if (typeof api.friendStatuses !== 'function') return
    let alive = true
    const load = () =>
      api.friendStatuses().then(
        (next) => {
          if (alive) setStatuses(next)
        },
        () => undefined,
      )
    void load()
    const timer = setInterval(() => void load(), REFRESH_MS)
    return () => {
      alive = false
      clearInterval(timer)
    }
  }, [api, friendCount])
  return statuses
}

/**
 * "Now", refreshed every minute, so a row's `HH:mm` turns into a weekday at midnight
 * without a reload. Uses the injected browser clock, not the timer globals.
 */
import { useEffect, useState } from 'react'
import { browserClock } from '../../app/platform'

export function useMinuteClock(): Date {
  const [now, setNow] = useState(() => new Date(browserClock.now()))
  useEffect(() => {
    let handle: unknown = null
    const tick = () => {
      const current = browserClock.now()
      setNow(new Date(current))
      handle = browserClock.setTimeout(tick, 60_000 - (current % 60_000))
    }
    handle = browserClock.setTimeout(tick, 60_000 - (browserClock.now() % 60_000))
    return () => browserClock.clearTimeout(handle)
  }, [])
  return now
}

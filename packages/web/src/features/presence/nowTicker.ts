/**
 * The ONE clock every relative presence text re-renders from (TG-107): a single timer for
 * the whole app instead of one per chat-list row.
 *
 * It ticks every `periodMs` while anybody listens, and additionally wakes at the exact
 * instant a `wakeSource` asks for — that is how a typing line disappears precisely when
 * its 5 s TTL runs out rather than up to one period later. Framework-free and clock-
 * injected, so the tests drive it with a fake clock.
 */
import type { CoreClock, CoreTimerHandle } from '@tg/core'

export interface WakeSource {
  subscribe(listener: () => void): () => void
  /** Earliest future instant (epoch ms, > now) at which derived text changes, or null. */
  nextWakeAt(now: number): number | null
}

export interface NowTicker {
  subscribe(listener: () => void): () => void
  /** Last tick while running; a live clock read while idle. */
  now(): number
}

export function createNowTicker(options: { clock: CoreClock; periodMs: number; wakeSource?: WakeSource }): NowTicker {
  const { clock, periodMs, wakeSource } = options
  const listeners = new Set<() => void>()
  let current = clock.now()
  let timer: CoreTimerHandle | null = null
  let detachWake: (() => void) | null = null

  const schedule = () => {
    if (timer !== null) clock.clearTimeout(timer)
    // Measured from the live clock: a source change can arrive long after the last tick.
    const at = clock.now()
    const wake = wakeSource?.nextWakeAt(at) ?? null
    const delay = wake === null ? periodMs : Math.max(0, Math.min(periodMs, wake - at))
    timer = clock.setTimeout(tick, delay)
  }

  function tick() {
    timer = null
    current = clock.now()
    for (const listener of [...listeners]) listener()
    if (listeners.size > 0) schedule()
  }

  const start = () => {
    current = clock.now()
    detachWake = wakeSource?.subscribe(schedule) ?? null
    schedule()
  }

  const stop = () => {
    detachWake?.()
    detachWake = null
    if (timer !== null) clock.clearTimeout(timer)
    timer = null
  }

  return {
    subscribe(listener) {
      listeners.add(listener)
      if (listeners.size === 1) start()
      return () => {
        if (!listeners.delete(listener)) return
        if (listeners.size === 0) stop()
      }
    },
    now: () => (listeners.size > 0 ? current : clock.now()),
  }
}

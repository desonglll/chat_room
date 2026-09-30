import { useEffect, useRef, useState, type RefObject } from 'react'

/**
 * Keeps an element mounted long enough for its exit animation to play.
 *
 * Without this, closing an overlay unmounts it in the same frame and the exit transition never
 * runs - which is the most common reason a hand-rolled modal "flashes" shut. The wait is driven by
 * `animationend` / `transitionend` with a safety timeout derived from the element's own computed
 * duration, so `prefers-reduced-motion` needs no special case: TG-009 has already collapsed the
 * duration to 1ms by the time this reads it, and the element unmounts in about 51ms.
 *
 * `present` becomes true DURING the render in which `open` becomes true, not in an effect. That
 * ordering is load-bearing: the overlay's surface must exist in the same commit, otherwise the
 * focus-trap and positioning effects run against a ref that is still null and never re-run. This
 * is the render-phase state update React documents for state derived from props.
 */
export type PresenceState = 'open' | 'closing'

function longestMs(value: string): number {
  let longest = 0
  for (const part of value.split(',')) {
    const trimmed = part.trim()
    if (trimmed === '') continue
    const numeric = Number.parseFloat(trimmed)
    if (!Number.isFinite(numeric)) continue
    longest = Math.max(longest, trimmed.endsWith('ms') ? numeric : numeric * 1000)
  }
  return longest
}

export interface Presence {
  /** Whether the consumer should render at all. */
  present: boolean
  /** Goes into a `data-tg-state` attribute so CSS can pick the exit animation. */
  state: PresenceState
}

export function usePresence(open: boolean, nodeRef: RefObject<HTMLElement | null>): Presence {
  const [exiting, setExiting] = useState(false)
  const lastOpen = useRef(open)

  if (lastOpen.current !== open) {
    lastOpen.current = open
    // Closing starts an exit; reopening during an exit cancels it rather than queueing a unmount.
    if (open) {
      if (exiting) setExiting(false)
    } else {
      setExiting(true)
    }
  }

  const present = open || exiting
  const state: PresenceState = open ? 'open' : 'closing'

  useEffect(() => {
    if (!exiting) return
    const node = nodeRef.current
    if (node === null) {
      setExiting(false)
      return
    }

    let settled = false
    const finish = () => {
      if (settled) return
      settled = true
      setExiting(false)
    }
    // Only the surface's own animation counts; a child's transition must not unmount the parent.
    const onEnd = (event: Event) => {
      if (event.target === node) finish()
    }

    node.addEventListener('animationend', onEnd)
    node.addEventListener('transitionend', onEnd)
    const styles = window.getComputedStyle(node)
    const budget = Math.max(longestMs(styles.animationDuration), longestMs(styles.transitionDuration)) + 50
    const timer = window.setTimeout(finish, budget)

    return () => {
      node.removeEventListener('animationend', onEnd)
      node.removeEventListener('transitionend', onEnd)
      window.clearTimeout(timer)
    }
  }, [exiting, nodeRef])

  return { present, state }
}

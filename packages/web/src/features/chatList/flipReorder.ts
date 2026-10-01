/**
 * TG-805: Telegram's chat list glides rows to their new place when a chat jumps to the top
 * on a new message, instead of the list snapping. FLIP: remember each row's offset, and after
 * React re-orders the DOM, start every moved row at its old offset and animate the transform
 * back to none (compositor-only, no layout per frame).
 *
 * Only re-orders animate: a different folder, search query or collapsed state is a different
 * list (`context`), whose first render just records positions. Reduced motion skips it.
 */
import { useLayoutEffect, useRef } from 'react'
import type { RefObject } from 'react'

const DURATION_MS = 250
const EASING = 'cubic-bezier(0.25, 0.1, 0.25, 1)'

/** Rows that moved between two layouts: `[key, old offset − new offset]`, only for rows in both. */
export function flipDeltas(
  previous: ReadonlyMap<string, number>,
  next: ReadonlyMap<string, number>,
): [string, number][] {
  const moved: [string, number][] = []
  for (const [key, top] of next) {
    const before = previous.get(key)
    if (before === undefined) continue
    const delta = before - top
    if (Math.abs(delta) >= 1) moved.push([key, delta])
  }
  return moved
}

function prefersReducedMotion(): boolean {
  return typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches
}

/** Animates `[data-flip-key]` children of `listRef` whenever `order` changes within one `context`. */
export function useFlipReorder(listRef: RefObject<HTMLElement | null>, order: string, context: string): void {
  const positions = useRef<{ context: string; tops: Map<string, number> } | null>(null)
  useLayoutEffect(() => {
    const list = listRef.current
    if (!list) return
    const tops = new Map<string, number>()
    for (const row of list.querySelectorAll<HTMLElement>(':scope > [data-flip-key]')) {
      tops.set(row.dataset.flipKey ?? '', row.offsetTop)
    }
    const previous = positions.current
    positions.current = { context, tops }
    if (!previous || previous.context !== context || prefersReducedMotion()) return
    for (const [key, delta] of flipDeltas(previous.tops, tops)) {
      const row = list.querySelector<HTMLElement>(`:scope > [data-flip-key="${CSS.escape(key)}"]`)
      row?.animate([{ transform: `translateY(${delta}px)` }, { transform: 'none' }], {
        duration: DURATION_MS,
        easing: EASING,
      })
    }
  }, [listRef, order, context])
}

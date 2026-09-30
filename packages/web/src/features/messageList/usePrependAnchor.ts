/**
 * Pixel-exact anchor guard for prepends (TG-101, found and verified by the browser
 * benchmark). Virtuoso compensates a prepend through its "upward scrolling" fix, which it
 * skips when the rendered range started at the list's first row both before and after the
 * change — exactly the case of a reader parked at the top while the page lands. The rows
 * then stay shifted down by the prepended height.
 *
 * The same guard makes "back to where you were" pixel-exact: Virtuoso's
 * `initialTopMostItemIndex` positions a remounted list from ESTIMATED row heights, so the
 * remembered row is pinned and corrected as soon as it is measured.
 *
 * The guard pins the topmost visible row right before the controller prepends, then — in a
 * ResizeObserver callback, i.e. after layout and BEFORE paint, and after Virtuoso's own
 * observers — restores that row's offset with a synchronous scrollTop correction. When
 * Virtuoso already compensated, the measured delta is 0 and nothing happens; the guard
 * never double-corrects. It stands down on the first user input so it never fights a
 * reader who starts scrolling.
 */
import { useCallback, useEffect, useRef } from 'react'

/** Long enough to cover Virtuoso's rAF-deferred unshift pipeline plus measurement. */
const GUARD_MS = 600
/** A return may remount the list and fetch nothing, but must wait for its first measure. */
const RESTORE_MS = 1_500

interface PinnedAnchor {
  key: string
  top: number
  until: number
}

function rowTop(scroller: HTMLElement, key: string): number | null {
  const row = scroller.querySelector<HTMLElement>(`[data-row-key="${CSS.escape(key)}"]`)
  return row ? row.getBoundingClientRect().top - scroller.getBoundingClientRect().top : null
}

export interface PrependAnchorGuard {
  /** Pin the topmost visible row; call right before rows are prepended. */
  pin(): void
  /**
   * Pin a remembered position ("back to where you were"): as soon as `key` is rendered,
   * scroll so its top sits `top` px below the viewport top. Survives a list remount.
   */
  pinAt(key: string, top: number): void
  /** Ref callback for Virtuoso's `scrollerRef`: attaches the guard to the scroller. */
  attach(element: HTMLElement | Window | null): void
  scroller: { current: HTMLElement | null }
}

export function usePrependAnchor(): PrependAnchorGuard {
  const pinned = useRef<PinnedAnchor | null>(null)
  const scroller = useRef<HTMLElement | null>(null)
  const detach = useRef<(() => void) | null>(null)

  const pin = useCallback(() => {
    const element = scroller.current
    if (!element) return
    const top = element.getBoundingClientRect().top
    for (const row of element.querySelectorAll<HTMLElement>('[data-row-key]')) {
      const rect = row.getBoundingClientRect()
      if (rect.bottom > top + 1) {
        pinned.current = { key: row.dataset.rowKey ?? '', top: rect.top - top, until: performance.now() + GUARD_MS }
        return
      }
    }
  }, [])

  const pinAt = useCallback((key: string, top: number) => {
    pinned.current = { key, top, until: performance.now() + RESTORE_MS }
  }, [])

  const attach = useCallback((element: HTMLElement | Window | null) => {
    detach.current?.()
    detach.current = null
    scroller.current = element instanceof HTMLElement ? element : null
    const target = scroller.current
    if (!target) return
    const restore = () => {
      const anchor = pinned.current
      if (!anchor) return
      if (performance.now() > anchor.until) {
        pinned.current = null
        return
      }
      const top = rowTop(target, anchor.key)
      if (top === null) return
      const delta = top - anchor.top
      if (Math.abs(delta) >= 1) target.scrollTop += delta
    }
    const standDown = () => {
      pinned.current = null
    }
    // Created after Virtuoso's observers, so delivered after them within a frame.
    const observer = new ResizeObserver(restore)
    observer.observe(target)
    const list = target.querySelector('[data-testid="virtuoso-item-list"]') ?? target.firstElementChild
    if (list) observer.observe(list)
    target.addEventListener('wheel', standDown, { passive: true })
    target.addEventListener('touchstart', standDown, { passive: true })
    target.addEventListener('keydown', standDown)
    target.addEventListener('pointerdown', standDown)
    detach.current = () => {
      observer.disconnect()
      target.removeEventListener('wheel', standDown)
      target.removeEventListener('touchstart', standDown)
      target.removeEventListener('keydown', standDown)
      target.removeEventListener('pointerdown', standDown)
    }
  }, [])

  useEffect(() => () => detach.current?.(), [])

  return { pin, pinAt, attach, scroller }
}

/**
 * Overscan that is widened while an older page is in flight (TG-101, found and verified by
 * the browser benchmark). When older rows land while the reader sits at the very top,
 * Virtuoso commits them, scrolls by their height in a rAF, and only recomputes its rendered
 * range from that scroll event one frame later. In between, the range is computed from the
 * OLD scrollTop against the NEW layout: it covers the prepended rows and stops above the
 * viewport, painting one blank frame. Rendering a page's worth of extra pixels below keeps
 * the real viewport inside that stale range.
 *
 * The widening must already be in place when the prepend commits: changing
 * `increaseViewportBy` in the same render as `firstItemIndex` breaks Virtuoso's unshift
 * compensation (measured: the anchor then moves by the page height). So it starts with the
 * request and ends BOOST_SETTLE_MS after the rows landed.
 */
import { useEffect, useState } from 'react'

export const OVERSCAN_TOP_PX = 800
export const OVERSCAN_BOTTOM_PX = 400
/** A page of rows at a generous per-row height, so tall (media) rows are still covered. */
export const PREPEND_BOOST_PX = 50 * 90
/** Longer than Virtuoso's rAF → scrollBy → scroll event → range update chain. */
const BOOST_SETTLE_MS = 200

export function usePrependOverscan(loadingOlder: boolean): { top: number; bottom: number } {
  const [lingering, setLingering] = useState(false)
  useEffect(() => {
    if (loadingOlder) {
      setLingering(true)
      return
    }
    const timer = setTimeout(() => setLingering(false), BOOST_SETTLE_MS)
    return () => clearTimeout(timer)
  }, [loadingOlder])
  const boosted = loadingOlder || lingering
  return { top: OVERSCAN_TOP_PX, bottom: OVERSCAN_BOTTOM_PX + (boosted ? PREPEND_BOOST_PX : 0) }
}

import { useCallback, useEffect, useLayoutEffect, useState, type RefObject } from 'react'
import { placeFloating, pointRect, type Placed, type Placement, type Rect } from './positioning'

/**
 * Measures an anchor and a floating element and keeps the floating element placed.
 *
 * The anchor is either an element or a viewport point (`ContextMenu` opens at the pointer). Both
 * reduce to a `Rect`, which is all `placeFloating` needs — so the geometry stays pure and this
 * hook only does measurement and re-measurement.
 */
export type AnchorTarget = RefObject<HTMLElement | null> | { x: number; y: number } | null

export interface AnchoredPositionOptions {
  active: boolean
  anchor: AnchorTarget
  floatingRef: RefObject<HTMLElement | null>
  placement: Placement
  offset: number
  padding: number
  /** Give the floating element the anchor's width, for a select-style dropdown. */
  matchAnchorWidth?: boolean | undefined
}

export interface AnchoredPosition extends Placed {
  /** Anchor width, exposed so the consumer can honour `matchAnchorWidth` in a style. */
  anchorWidth: number
  measured: boolean
}

const UNMEASURED: AnchoredPosition = { x: 0, y: 0, placement: 'bottom', anchorWidth: 0, measured: false }

function same(a: AnchoredPosition, b: AnchoredPosition): boolean {
  return (
    a.x === b.x &&
    a.y === b.y &&
    a.placement === b.placement &&
    a.anchorWidth === b.anchorWidth &&
    a.measured === b.measured
  )
}

function anchorRect(anchor: AnchorTarget): Rect | null {
  if (anchor === null) return null
  if ('x' in anchor && 'y' in anchor) return pointRect(anchor.x, anchor.y)
  const element = anchor.current
  if (element === null) return null
  const rect = element.getBoundingClientRect()
  return { x: rect.left, y: rect.top, width: rect.width, height: rect.height }
}

export function useAnchoredPosition(options: AnchoredPositionOptions): AnchoredPosition {
  const { active, anchor, floatingRef, placement, offset, padding } = options
  const [position, setPosition] = useState<AnchoredPosition>(UNMEASURED)

  const measure = useCallback(() => {
    const floating = floatingRef.current
    const rect = anchorRect(anchor)
    if (floating === null || rect === null) return
    const placed = placeFloating({
      anchor: rect,
      floating: { width: floating.offsetWidth, height: floating.offsetHeight },
      viewport: { width: window.innerWidth, height: window.innerHeight },
      placement,
      offset,
      padding,
    })
    const next: AnchoredPosition = { ...placed, anchorWidth: rect.width, measured: true }
    // Bail out when nothing moved. Without this, a consumer that builds its `anchor` inline
    // (`anchor={{ current: node }}`) changes `measure`'s identity on every render, the layout
    // effect re-runs, a fresh object is stored, and the component re-renders forever.
    setPosition((current) => (same(current, next) ? current : next))
  }, [anchor, floatingRef, placement, offset, padding])

  // Layout effect so the overlay is never painted at 0,0 before being moved.
  useLayoutEffect(() => {
    if (!active) {
      setPosition((current) => (current.measured ? UNMEASURED : current))
      return
    }
    measure()
  }, [active, measure])

  useEffect(() => {
    if (!active) return
    // `true` on scroll: an anchor inside a scrolled panel moves without the window scrolling.
    window.addEventListener('scroll', measure, true)
    window.addEventListener('resize', measure)
    return () => {
      window.removeEventListener('scroll', measure, true)
      window.removeEventListener('resize', measure)
    }
  }, [active, measure])

  return position
}

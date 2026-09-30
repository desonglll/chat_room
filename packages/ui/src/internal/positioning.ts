/**
 * Anchored-overlay placement: pure geometry, no DOM, no dependency.
 *
 * `Popover`, `Menu`, `ContextMenu` and `Tooltip` all measure with `getBoundingClientRect()` and
 * then call `placeFloating`, so flipping and clamping are defined once and unit tested against
 * fixtures. This is also why the package does not need floating-ui: the only two behaviours
 * Telegram's overlays show are "flip to the other side when you do not fit" and "stay inside the
 * viewport", both of which are a dozen lines.
 */
export type Side = 'top' | 'right' | 'bottom' | 'left'
export type Align = 'start' | 'center' | 'end'

/** The 12 placements, in `<side>` or `<side>-<align>` form. `<side>` alone means centred. */
export type Placement =
  | Side
  | 'top-start'
  | 'top-end'
  | 'right-start'
  | 'right-end'
  | 'bottom-start'
  | 'bottom-end'
  | 'left-start'
  | 'left-end'

export interface Rect {
  x: number
  y: number
  width: number
  height: number
}

export interface Size {
  width: number
  height: number
}

export interface PlaceInput {
  /** Anchor rectangle in viewport coordinates. A pointer point is a zero-size rect. */
  anchor: Rect
  floating: Size
  viewport: Size
  placement: Placement
  /** Gap between the anchor edge and the overlay edge. */
  offset: number
  /** Minimum distance kept from the viewport edges. */
  padding: number
}

export interface Placed {
  x: number
  y: number
  /** The placement actually used, which differs from the request when the side was flipped. */
  placement: Placement
}

const OPPOSITE: Record<Side, Side> = { top: 'bottom', bottom: 'top', left: 'right', right: 'left' }

export function parsePlacement(placement: Placement): { side: Side; align: Align } {
  const dash = placement.indexOf('-')
  if (dash === -1) return { side: placement as Side, align: 'center' }
  return { side: placement.slice(0, dash) as Side, align: placement.slice(dash + 1) as Align }
}

export function formatPlacement(side: Side, align: Align): Placement {
  return (align === 'center' ? side : `${side}-${align}`) as Placement
}

function crossAxis(anchorStart: number, anchorSize: number, floatingSize: number, align: Align): number {
  if (align === 'start') return anchorStart
  if (align === 'end') return anchorStart + anchorSize - floatingSize
  return anchorStart + (anchorSize - floatingSize) / 2
}

function coordsFor(side: Side, align: Align, input: PlaceInput): { x: number; y: number } {
  const { anchor, floating, offset } = input
  switch (side) {
    case 'top':
      return { x: crossAxis(anchor.x, anchor.width, floating.width, align), y: anchor.y - floating.height - offset }
    case 'bottom':
      return { x: crossAxis(anchor.x, anchor.width, floating.width, align), y: anchor.y + anchor.height + offset }
    case 'left':
      return { x: anchor.x - floating.width - offset, y: crossAxis(anchor.y, anchor.height, floating.height, align) }
    case 'right':
      return { x: anchor.x + anchor.width + offset, y: crossAxis(anchor.y, anchor.height, floating.height, align) }
  }
}

function fitsOnMainAxis(side: Side, input: PlaceInput): boolean {
  const { anchor, floating, viewport, offset, padding } = input
  switch (side) {
    case 'top':
      return anchor.y - floating.height - offset >= padding
    case 'bottom':
      return anchor.y + anchor.height + offset + floating.height <= viewport.height - padding
    case 'left':
      return anchor.x - floating.width - offset >= padding
    case 'right':
      return anchor.x + anchor.width + offset + floating.width <= viewport.width - padding
  }
}

function clamp(value: number, size: number, extent: number, padding: number): number {
  const max = extent - size - padding
  // When the overlay is larger than the viewport, pin it to the leading edge rather than
  // producing a negative-width window the user cannot scroll to.
  if (max < padding) return padding
  return Math.min(Math.max(value, padding), max)
}

export function placeFloating(input: PlaceInput): Placed {
  const { side, align } = parsePlacement(input.placement)
  const flipped = !fitsOnMainAxis(side, input) && fitsOnMainAxis(OPPOSITE[side], input)
  const finalSide = flipped ? OPPOSITE[side] : side
  const raw = coordsFor(finalSide, align, input)
  return {
    x: clamp(raw.x, input.floating.width, input.viewport.width, input.padding),
    y: clamp(raw.y, input.floating.height, input.viewport.height, input.padding),
    placement: formatPlacement(finalSide, align),
  }
}

/** A pointer position expressed as the zero-size anchor rect that `placeFloating` expects. */
export function pointRect(x: number, y: number): Rect {
  return { x, y, width: 0, height: 0 }
}

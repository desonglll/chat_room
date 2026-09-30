/**
 * Geometry of the zoomable stage. Pure numbers, no DOM, so bounds and gestures are unit
 * tested (test/zoomGeometry.test.ts).
 *
 * Model: the medium is laid out at its fitted base size `base` in the middle of `viewport`
 * and drawn with `translate(x, y) scale(scale)` around its centre. Its on-screen box is
 * `scale · base`, centred at `viewport/2 + (x, y)`.
 */

export interface Size {
  width: number
  height: number
}

export interface Point {
  x: number
  y: number
}

export interface Rect extends Point, Size {}

export interface ZoomTransform {
  scale: number
  x: number
  y: number
}

export const IDENTITY: ZoomTransform = { scale: 1, x: 0, y: 0 }
export const MIN_SCALE = 1
export const MAX_SCALE = 4
export const DOUBLE_TAP_SCALE = 2.5
/** A pinch may overshoot the limits this far before resistance stops it; release settles back. */
const PINCH_SLACK = 0.35

export const clamp = (value: number, min: number, max: number): number => Math.min(max, Math.max(min, value))

/** Fit `natural` inside `box` without upscaling (the stage draws small media at 1:1). */
export function fitContain(natural: Size, box: Size): Size {
  if (natural.width <= 0 || natural.height <= 0) return { width: 0, height: 0 }
  const ratio = Math.min(1, box.width / natural.width, box.height / natural.height)
  return { width: natural.width * ratio, height: natural.height * ratio }
}

/** `size` centred in `box`, as a rect relative to `box`'s origin. */
export function centreIn(size: Size, box: Rect): Rect {
  return { x: box.x + (box.width - size.width) / 2, y: box.y + (box.height - size.height) / 2, ...size }
}

/** How far the medium may travel from centre on each axis at `scale`: 0 until it overflows. */
export function panLimits(scale: number, base: Size, viewport: Size): Point {
  return {
    x: Math.max(0, (scale * base.width - viewport.width) / 2),
    y: Math.max(0, (scale * base.height - viewport.height) / 2),
  }
}

/** The legal transform nearest to `t`: scale within limits, no empty gap at any edge. */
export function clampTransform(t: ZoomTransform, base: Size, viewport: Size): ZoomTransform {
  const scale = clamp(t.scale, MIN_SCALE, MAX_SCALE)
  const limit = panLimits(scale, base, viewport)
  return { scale, x: clamp(t.x, -limit.x, limit.x), y: clamp(t.y, -limit.y, limit.y) }
}

/**
 * Change the scale while keeping the content point under `focus` (relative to the viewport
 * centre) fixed on screen — how both wheel and pinch zoom feel anchored to the cursor.
 */
export function zoomAround(t: ZoomTransform, nextScale: number, focus: Point): ZoomTransform {
  const ratio = nextScale / t.scale
  return { scale: nextScale, x: focus.x - (focus.x - t.x) * ratio, y: focus.y - (focus.y - t.y) * ratio }
}

/**
 * iOS rubber band: past a bound the content still follows the finger, ever more reluctantly,
 * and never beyond `dimension`. Inside the bounds it is the identity.
 */
export function rubberBand(value: number, min: number, max: number, dimension: number): number {
  const band = (overshoot: number) => (1 - 1 / ((overshoot * 0.55) / Math.max(1, dimension) + 1)) * dimension
  if (value < min) return min - band(min - value)
  if (value > max) return max + band(value - max)
  return value
}

/** A pan while zoomed: free inside the limits, rubber-banded beyond them. */
export function dragTransform(start: ZoomTransform, delta: Point, base: Size, viewport: Size): ZoomTransform {
  const limit = panLimits(start.scale, base, viewport)
  return {
    scale: start.scale,
    x: rubberBand(start.x + delta.x, -limit.x, limit.x, viewport.width),
    y: rubberBand(start.y + delta.y, -limit.y, limit.y, viewport.height),
  }
}

/** Pinch scale with soft limits, so the gesture never hits a wall mid-pinch. */
export function pinchScale(startScale: number, startDistance: number, distance: number): number {
  if (startDistance <= 0) return startScale
  return clamp((startScale * distance) / startDistance, MIN_SCALE * (1 - PINCH_SLACK), MAX_SCALE * (1 + PINCH_SLACK))
}

/**
 * One wheel event's new scale. A trackpad pinch arrives as a wheel event with `ctrlKey`
 * and small deltas, so it is amplified; line-mode deltas are converted to pixels.
 */
export function wheelScale(scale: number, deltaY: number, deltaMode: number, ctrlKey: boolean): number {
  const pixels = deltaMode === 1 ? deltaY * 16 : deltaMode === 2 ? deltaY * 400 : deltaY
  return clamp(scale * Math.exp(-pixels * (ctrlKey ? 0.01 : 0.002)), MIN_SCALE, MAX_SCALE)
}

/** Double-click / double-tap toggles between fitted and a comfortable close-up. */
export function doubleTapScale(scale: number): number {
  return scale > MIN_SCALE + 0.01 ? MIN_SCALE : DOUBLE_TAP_SCALE
}

export const isZoomed = (t: ZoomTransform): boolean => t.scale > MIN_SCALE + 0.01

export type SwipeAxis = 'x' | 'y'

/** Pointer slop before a drag at fitted scale commits to one axis. */
export const SWIPE_SLOP = 8

export function lockAxis(dx: number, dy: number): SwipeAxis | null {
  if (Math.hypot(dx, dy) < SWIPE_SLOP) return null
  return Math.abs(dx) >= Math.abs(dy) ? 'x' : 'y'
}

export type SwipeOutcome = 'next' | 'prev' | 'dismiss' | 'cancel'

export interface SwipeInput {
  axis: SwipeAxis
  /** Displacement (px) and release velocity (px/ms). */
  dx: number
  dy: number
  vx: number
  vy: number
  viewport: Size
}

/**
 * Where a fitted-scale drag lands. Horizontal: far enough or fast enough in one direction
 * flips to the neighbour (dragging left reveals the newer item on the right). Vertical: the
 * Telegram swipe-to-dismiss, either direction.
 */
export function resolveSwipe({ axis, dx, dy, vx, vy, viewport }: SwipeInput): SwipeOutcome {
  if (axis === 'x') {
    const flick = Math.abs(vx) > 0.5 && Math.sign(vx) === Math.sign(dx)
    if (Math.abs(dx) > viewport.width * 0.2 || (flick && Math.abs(dx) > SWIPE_SLOP)) return dx < 0 ? 'next' : 'prev'
    return 'cancel'
  }
  const flick = Math.abs(vy) > 0.6 && Math.sign(vy) === Math.sign(dy)
  return Math.abs(dy) > viewport.height * 0.15 || (flick && Math.abs(dy) > SWIPE_SLOP) ? 'dismiss' : 'cancel'
}

/** Backdrop opacity while dragging to dismiss: fades to 20% over half the viewport height. */
export function dismissOpacity(dy: number, viewportHeight: number): number {
  return 1 - 0.8 * clamp(Math.abs(dy) / Math.max(1, viewportHeight * 0.5), 0, 1)
}

export function lerp(from: number, to: number, t: number): number {
  return from + (to - from) * t
}

export function lerpRect(from: Rect, to: Rect, t: number): Rect {
  return {
    x: lerp(from.x, to.x, t),
    y: lerp(from.y, to.y, t),
    width: lerp(from.width, to.width, t),
    height: lerp(from.height, to.height, t),
  }
}

/**
 * The swipe-left-to-archive gesture's decisions, pure so they are testable without a DOM:
 * axis lock (a vertical drag is a scroll and is left to the browser), the row offset with a
 * rubber band past the full width, and whether the release commits. The component
 * (`SwipeArchive.tsx`) only feeds pointer coordinates in and animates what comes out.
 */

/** Travel before the gesture picks an axis; below the ContextMenu long-press slop × 2. */
export const AXIS_LOCK_PX = 10
/** Share of the row width past which a release archives. Telegram commits at about a third. */
export const COMMIT_FRACTION = 0.35
/** A flick commits from a shorter distance: px/ms leftwards. */
export const FLICK_VELOCITY = 0.6

export type SwipeAxis = 'pending' | 'horizontal' | 'vertical'

/** Lock the axis once the finger has travelled far enough; only a leftward drag locks horizontal. */
export function lockAxis(dx: number, dy: number): SwipeAxis {
  if (Math.abs(dx) < AXIS_LOCK_PX && Math.abs(dy) < AXIS_LOCK_PX) return 'pending'
  return dx < 0 && Math.abs(dx) > Math.abs(dy) * 1.2 ? 'horizontal' : 'vertical'
}

/** Row offset for a finger delta: never right of rest, rubber-banded beyond `-width`. */
export function swipeOffset(dx: number, width: number): number {
  if (dx >= 0) return 0
  if (-dx <= width) return dx
  const over = -dx - width
  return -width - over / (1 + over / Math.max(width, 1))
}

export function commitsArchive(offset: number, width: number, velocity: number): boolean {
  if (width <= 0) return false
  if (-offset >= width * COMMIT_FRACTION) return true
  return velocity <= -FLICK_VELOCITY && -offset >= AXIS_LOCK_PX * 3
}

/** 0 at rest → 1 at the commit line; drives the revealed icon's scale. */
export function swipeProgress(offset: number, width: number): number {
  if (width <= 0) return 0
  return Math.min(1, Math.max(0, -offset / (width * COMMIT_FRACTION)))
}

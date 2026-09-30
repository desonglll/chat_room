/**
 * Index arithmetic over the viewer's media list. The current item is addressed by its
 * attachment id, never by index, so prepending an older page does not move the view. Pure.
 */
import type { MediaItem } from './mediaItem'

export type Direction = -1 | 1

/** How close to the oldest loaded item the viewer asks for the next older page. */
export const OLDER_PAGE_THRESHOLD = 4

export function indexOfItem(items: readonly MediaItem[], attachmentId: string): number {
  return items.findIndex((item) => item.attachmentId === attachmentId)
}

/** The neighbour in `direction` (-1 older / left, +1 newer / right), or null at an end. */
export function neighbourId(items: readonly MediaItem[], attachmentId: string, direction: Direction): string | null {
  const index = indexOfItem(items, attachmentId)
  if (index < 0) return null
  return items[index + direction]?.attachmentId ?? null
}

/** After deleting the current item: prefer the newer neighbour, then the older one. */
export function successorAfterRemoval(items: readonly MediaItem[], attachmentId: string): string | null {
  return neighbourId(items, attachmentId, 1) ?? neighbourId(items, attachmentId, -1)
}

export function shouldLoadOlder(index: number, hasOlder: boolean): boolean {
  return hasOlder && index >= 0 && index < OLDER_PAGE_THRESHOLD
}

/**
 * The slice of the thumbnail strip worth rendering: `radius` either side of the current
 * item, shifted inward at the ends so the strip stays full. `[start, end)`.
 */
export function stripWindow(length: number, index: number, radius: number): { start: number; end: number } {
  const size = Math.min(length, radius * 2 + 1)
  const start = Math.max(0, Math.min(index - radius, length - size))
  return { start, end: start + size }
}

/** "3 / 12" only when the whole list is known; a partial count would be a lie. */
export function positionLabel(index: number, length: number, complete: boolean): string {
  return complete && index >= 0 ? `${index + 1} / ${length}` : ''
}

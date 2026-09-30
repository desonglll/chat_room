/**
 * Telegram's ripple geometry. See docs/devlog/TG-010.md § Decisions for the comparison with
 * Material; the three properties this function encodes are:
 *
 *  1. the circle is centred on the pointer and STAYS there. Material's ripple translates from
 *     the pointer towards the element centre as it grows; Telegram's does not move at all.
 *  2. the diameter is `max(width, height)` of the host, not the diagonal, so the ripple is NOT
 *     guaranteed to reach the far corner of a non-square control. That under-coverage is
 *     Telegram's look; sizing for guaranteed coverage is Material's.
 *  3. scale runs `--tg-ripple-scale-start` -> `--tg-ripple-scale-end` (0 -> 2) over
 *     `--tg-ripple-duration` (700ms), roughly three times Material's, and it always runs to
 *     completion instead of being cut short on pointer release.
 */
export interface RippleInput {
  /** Host element size in CSS pixels. */
  host: { width: number; height: number }
  /** Pointer position relative to the host's top-left corner. */
  pointer: { x: number; y: number }
}

export interface RippleGeometry {
  /** Offsets for the un-scaled circle, relative to the host's padding box. */
  left: number
  top: number
  diameter: number
}

export function rippleGeometry({ host, pointer }: RippleInput): RippleGeometry {
  const diameter = Math.max(host.width, host.height)
  return { left: pointer.x - diameter / 2, top: pointer.y - diameter / 2, diameter }
}

/** Keyboard and programmatic activation has no pointer, so the origin is the host centre. */
export function hostCentre(host: { width: number; height: number }): { x: number; y: number } {
  return { x: host.width / 2, y: host.height / 2 }
}

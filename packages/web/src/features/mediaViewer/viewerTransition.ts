/**
 * Open/close choreography. Two shapes:
 *
 *  - `fly`: the shared-element zoom. A lone "flight" element carrying a snapshot of the
 *    medium travels between the thumbnail's rect and the stage's rect, morphing size and
 *    corner radii, while the backdrop fades. The stage stays hidden meanwhile, so exactly
 *    one copy of the medium is ever visible.
 *  - `fade`: when there is no thumbnail on screen (scrolled away, virtualised, opened from
 *    elsewhere) or under reduced motion — backdrop and stage fade; the stage also scales
 *    slightly unless motion is reduced.
 *
 * No white frame is possible by construction: the backdrop is the dark `--tg-overlay-strong`
 * token from its first paint, and the flight shows the same already-decoded image the
 * bubble shows (a video contributes a canvas still of its current frame).
 */
import type { Rect } from './zoomGeometry'
import { lerp, lerpRect } from './zoomGeometry'
import type { Progress } from './viewerMotion'
import { runProgress } from './viewerMotion'

export interface TransitionElements {
  backdrop: HTMLElement
  stage: HTMLElement
  flight: HTMLElement
  flightImage: HTMLImageElement
}

export type Radii = readonly [number, number, number, number]

export const SQUARE: Radii = [0, 0, 0, 0]

export interface Flight {
  src: string
  from: Rect
  to: Rect
  radii: { from: Radii; to: Radii }
  backdrop: { from: number; to: number }
}

export function fly(elements: TransitionElements, flight: Flight, reduced: boolean): Progress {
  const { flight: box, flightImage, backdrop } = elements
  flightImage.src = flight.src
  box.hidden = false
  const paint = (t: number) => {
    const rect = lerpRect(flight.from, flight.to, t)
    box.style.transform = `translate3d(${rect.x}px, ${rect.y}px, 0)`
    box.style.width = `${rect.width}px`
    box.style.height = `${rect.height}px`
    box.style.borderRadius = flight.radii.from.map((r, i) => `${lerp(r, flight.radii.to[i] ?? 0, t)}px`).join(' ')
    backdrop.style.opacity = String(lerp(flight.backdrop.from, flight.backdrop.to, t))
  }
  paint(0)
  return runProgress(paint, { reduced, from: backdrop })
}

export function endFlight(elements: TransitionElements): void {
  elements.flight.hidden = true
  elements.flightImage.removeAttribute('src')
}

/** Stage + backdrop fade; `from` is the backdrop's current opacity (a dismiss drag dims it). */
export function fade(elements: TransitionElements, direction: 'in' | 'out', from: number, reduced: boolean): Progress {
  const { stage, backdrop } = elements
  const to = direction === 'in' ? 1 : 0
  const paint = (t: number) => {
    const visible = direction === 'in' ? t : 1 - t
    backdrop.style.opacity = String(lerp(from, to, t))
    stage.style.opacity = String(visible)
    stage.style.transform = reduced ? '' : `scale(${lerp(0.94, 1, visible)})`
  }
  paint(0)
  return runProgress(paint, { reduced, from: backdrop })
}

/**
 * A still of what `element` shows, for the flight: an image's own URL (already decoded), or
 * a canvas frame of a video. Null when there is nothing drawable yet.
 */
export function snapshotOf(element: Element | null): string | null {
  const media = element?.matches('img, video') ? element : (element?.querySelector('img, video') ?? null)
  if (media instanceof HTMLImageElement)
    return media.complete && media.naturalWidth > 0 ? media.currentSrc || media.src : null
  if (!(media instanceof HTMLVideoElement) || media.videoWidth === 0 || media.readyState < 2) return null
  try {
    const canvas = document.createElement('canvas')
    canvas.width = media.videoWidth
    canvas.height = media.videoHeight
    canvas.getContext('2d')?.drawImage(media, 0, 0)
    return canvas.toDataURL('image/jpeg', 0.85)
  } catch {
    return null
  }
}

export function prefersReducedMotion(): boolean {
  return typeof window !== 'undefined' && window.matchMedia('(prefers-reduced-motion: reduce)').matches
}

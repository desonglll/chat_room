/**
 * Finding the bubble thumbnail a medium flies from and back to. The bubble (TG-103) renders
 * the media frame as `.tg-bubble__media` around an `<img>`/`<video>` whose `src` is the
 * attachment's `download_url`, which is unique per attachment — so the URL is the join key
 * and the bubble needs no extra attribute. A virtualised-away or scrolled-off thumbnail is
 * simply not found, and the viewer fades instead of flying.
 */
import type { Rect, Size } from './zoomGeometry'

export interface Thumbnail {
  frame: HTMLElement
  rect: Rect
  natural: Size | null
  /** Corner radii (tl, tr, br, bl) in px, so the flight starts from the bubble's shape. */
  radii: [number, number, number, number]
}

export function rectOf(element: Element): Rect {
  const box = element.getBoundingClientRect()
  return { x: box.left, y: box.top, width: box.width, height: box.height }
}

function onScreen(rect: Rect): boolean {
  return (
    rect.width > 0 &&
    rect.height > 0 &&
    rect.x + rect.width > 0 &&
    rect.y + rect.height > 0 &&
    rect.x < window.innerWidth &&
    rect.y < window.innerHeight
  )
}

export function naturalSize(media: Element | null): Size | null {
  if (media instanceof HTMLImageElement && media.naturalWidth > 0)
    return { width: media.naturalWidth, height: media.naturalHeight }
  if (media instanceof HTMLVideoElement && media.videoWidth > 0)
    return { width: media.videoWidth, height: media.videoHeight }
  return null
}

function radiiOf(element: Element): Thumbnail['radii'] {
  const style = window.getComputedStyle(element)
  const px = (value: string) => Number.parseFloat(value) || 0
  return [
    px(style.borderTopLeftRadius),
    px(style.borderTopRightRadius),
    px(style.borderBottomRightRadius),
    px(style.borderBottomLeftRadius),
  ]
}

export function findThumbnail(url: string): Thumbnail | null {
  if (typeof document === 'undefined' || url === '') return null
  for (const frame of document.querySelectorAll<HTMLElement>('.tg-bubble__media')) {
    if (frame.hasAttribute('data-veiled')) continue
    const media = frame.querySelector('img, video')
    if (media?.getAttribute('src') !== url) continue
    const rect = rectOf(frame)
    if (!onScreen(rect)) continue
    return { frame, rect, natural: naturalSize(media), radii: radiiOf(frame) }
  }
  return null
}

/**
 * Hide the thumbnail while its copy is in the viewer (Telegram does: the photo "lifts out" of
 * the bubble). Returns the undo.
 */
export function hideThumbnail(frame: HTMLElement): () => void {
  const previous = frame.style.visibility
  frame.style.visibility = 'hidden'
  return () => {
    frame.style.visibility = previous
  }
}

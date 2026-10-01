/**
 * The viewer's motion state machine: `enter` (shared-element flight or fade in) → `open` →
 * `exit` (flight back to the current item's thumbnail, or fade out) → `onClosed`. Also the
 * stage track's drag/release for swipe navigation and swipe-to-dismiss.
 *
 * React state changes that must land in the same frame as an imperative style write go
 * through `flushSync`; otherwise React would commit one frame late and the medium would
 * blink out between the flight and the stage.
 */
import { useCallback, useLayoutEffect, useRef, useState, type RefObject } from 'react'
import { flushSync } from 'react-dom'
import type { MediaItem } from './mediaItem'
import type { MediaViewerRequest } from './mediaViewerStore'
import type { Progress } from './viewerMotion'
import { runProgress } from './viewerMotion'
import type { Size } from './zoomGeometry'
import { centreIn, dismissOpacity, fitContain, lerp } from './zoomGeometry'
import { findThumbnail, naturalSize, rectOf } from './thumbnailLookup'
import { SQUARE, endFlight, fade, fly, prefersReducedMotion, snapshotOf } from './viewerTransition'
import type { SwipeOffset } from './useStageGestures'

export type ViewerPhase = 'enter' | 'open' | 'exit'

export interface ViewerRefs {
  root: RefObject<HTMLDivElement | null>
  backdrop: RefObject<HTMLDivElement | null>
  stage: RefObject<HTMLDivElement | null>
  track: RefObject<HTMLDivElement | null>
  flight: RefObject<HTMLDivElement | null>
  flightImage: RefObject<HTMLImageElement | null>
}

interface Options {
  request: MediaViewerRequest
  current: MediaItem | null
  mediaRef: RefObject<HTMLImageElement | null>
  onClosed(): void
}

export function useViewerMotion({ request, current, mediaRef, onClosed }: Options) {
  const refs: ViewerRefs = {
    root: useRef<HTMLDivElement>(null),
    backdrop: useRef<HTMLDivElement>(null),
    stage: useRef<HTMLDivElement>(null),
    track: useRef<HTMLDivElement>(null),
    flight: useRef<HTMLDivElement>(null),
    flightImage: useRef<HTMLImageElement>(null),
  }
  const [reduced] = useState(prefersReducedMotion)
  const [phase, setPhase] = useState<ViewerPhase>('enter')
  const [flying, setFlying] = useState(false)
  const running = useRef<Progress | null>(null)
  const naturals = useRef(new Map<string, Size>())
  const trackAt = useRef({ x: 0, y: 0 })
  const latest = useRef({ current, onClosed })
  latest.current = { current, onClosed }

  const elements = () => {
    const { backdrop, stage, flight, flightImage } = refs
    if (!backdrop.current || !stage.current || !flight.current || !flightImage.current) return null
    return {
      backdrop: backdrop.current,
      stage: stage.current,
      flight: flight.current,
      flightImage: flightImage.current,
    }
  }

  const play = (progress: Progress) => {
    running.current?.stop()
    running.current = progress
    return progress.finished
  }

  // Enter — once, before the first paint, so the thumbnail's copy is the first thing drawn.
  useLayoutEffect(() => {
    const els = elements()
    if (!els) return
    const item = latest.current.current
    const thumbnail = item ? findThumbnail(item.previewUrl) : null
    const natural = item && thumbnail?.natural ? flightNatural(item, thumbnail.natural) : null
    if (item && natural) naturals.current.set(item.attachmentId, natural)
    const source = request.sourceRect ?? thumbnail?.rect ?? null
    const src = snapshotOf(thumbnail?.frame ?? null)
    if (!reduced && item && thumbnail && source && natural && src) {
      const stageBox = rectOf(els.stage)
      const to = centreIn(fitContain(natural, stageBox), stageBox)
      setFlying(true)
      const flight = {
        src,
        from: source,
        to,
        radii: { from: thumbnail.radii, to: SQUARE },
        backdrop: { from: 0, to: 1 },
      }
      void play(fly(els, flight, false)).then(async (done) => {
        if (!done) return
        // Swap only once the stage's copy can paint, or it would blink. With a thumbnail that
        // copy is the preview layer (already cached) — waiting for the original instead would
        // hold the flight mid-air for the whole download on a slow phone (TG-1302).
        await (stagePreview(els.stage) ?? mediaRef.current)?.decode().catch(() => undefined)
        flushSync(() => setFlying(false))
        endFlight(els)
      })
    } else {
      play(fade(els, 'in', 0, reduced))
    }
    const frame = requestAnimationFrame(() => setPhase((now) => (now === 'enter' ? 'open' : now)))
    return () => {
      cancelAnimationFrame(frame)
      running.current?.stop()
    }
    // Deliberately once: the entrance belongs to the item the viewer opened on.
  }, [])

  const close = useCallback(() => {
    const els = elements()
    if (!els) return latest.current.onClosed()
    if (refs.root.current?.dataset.phase === 'exit') return
    const item = latest.current.current
    const media =
      item?.kind === 'image'
        ? mediaRef.current
        : els.stage.querySelector<HTMLElement>('[data-offset="0"] [data-mv-video]')
    const thumbnail = item ? findThumbnail(item.previewUrl) : null
    const src = snapshotOf(media)
    const backdropNow = Number.parseFloat(els.backdrop.style.opacity || '1')
    running.current?.stop()
    flushSync(() => setPhase('exit'))
    let finished: Promise<boolean>
    if (!reduced && media && thumbnail && src) {
      const from = rectOf(media)
      const flight = {
        src,
        from,
        to: thumbnail.rect,
        radii: { from: SQUARE, to: thumbnail.radii },
        backdrop: { from: backdropNow, to: 0 },
      }
      finished = play(fly(els, flight, false))
      flushSync(() => setFlying(true))
    } else {
      finished = play(fade(els, 'out', backdropNow, reduced))
    }
    // Closing is final: even an interrupted exit ends in `onClosed`.
    void finished.then(() => latest.current.onClosed())
    // `refs` members are stable ref objects.
  }, [reduced])

  const dragTrack = (offset: SwipeOffset, hasNeighbour: boolean) => {
    const track = refs.track.current
    const stage = refs.stage.current
    if (!track || !stage) return
    running.current?.stop()
    const x = offset.axis === 'x' ? (hasNeighbour ? offset.dx : offset.dx * 0.3) : 0
    const y = offset.axis === 'y' ? offset.dy : 0
    trackAt.current = { x, y }
    track.style.transform = `translate3d(${x}px, ${y}px, 0)`
    if (refs.backdrop.current) refs.backdrop.current.style.opacity = String(dismissOpacity(y, stage.clientHeight))
  }

  /** Finish a swipe: slide to the neighbour (then `onArrive`) or spring back. */
  const releaseTrack = (_offset: SwipeOffset, direction: -1 | 0 | 1, onArrive: () => void) => {
    const track = refs.track.current
    const stage = refs.stage.current
    const backdrop = refs.backdrop.current
    if (!track || !stage || !backdrop) return
    const from = trackAt.current
    const toX = direction === 0 ? 0 : -direction * stage.clientWidth
    const backdropFrom = Number.parseFloat(backdrop.style.opacity || '1')
    const settle = () => {
      if (direction !== 0) flushSync(onArrive)
      track.style.transform = ''
      trackAt.current = { x: 0, y: 0 }
    }
    if (reduced) {
      backdrop.style.opacity = '1'
      return settle()
    }
    const paint = (t: number) => {
      track.style.transform = `translate3d(${lerp(from.x, toX, t)}px, ${lerp(from.y, 0, t)}px, 0)`
      backdrop.style.opacity = String(lerp(backdropFrom, 1, t))
    }
    void play(runProgress(paint, { reduced, from: stage })).then((done) => done && settle())
  }

  const naturalOf = (item: MediaItem): Size | null => {
    const known = naturals.current.get(item.attachmentId)
    if (known) return known
    const thumbnail = findThumbnail(item.previewUrl)
    return thumbnail ? naturalSize(thumbnail.frame.querySelector('video')) : null
  }

  return { refs, phase, flying, reduced, close, dragTrack, releaseTrack, naturalOf }
}

/**
 * TG-1302: when the bubble shows a server thumbnail, its pixel size says nothing about the
 * original's — only the aspect ratio carries over. Land the flight where the stage's preview
 * layer fills (an aspect-only "huge" natural), which is where a photo larger than the screen
 * ends up anyway, so the swap to the original does not jump.
 */
function flightNatural(item: MediaItem, natural: Size): Size {
  if (item.previewUrl === item.url || natural.width <= 0 || natural.height <= 0) return natural
  const scale = 100_000 / Math.max(natural.width, natural.height)
  return { width: natural.width * scale, height: natural.height * scale }
}

function stagePreview(stage: HTMLElement): HTMLImageElement | null {
  return stage.querySelector<HTMLImageElement>('.tg-mv__slide[data-offset="0"] .tg-mv__media--preview')
}

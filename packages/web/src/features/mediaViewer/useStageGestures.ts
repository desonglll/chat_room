/**
 * Every pointer gesture on the stage, in one place, because they compete for the same
 * pointers: pinch and wheel zoom, pan while zoomed (rubber-banded, settling inside the
 * bounds on release), double-tap/double-click zoom toggle, and — at fitted scale — the
 * horizontal swipe to the neighbour and the vertical swipe to dismiss. A tap outside the
 * medium closes. The math is `zoomGeometry.ts`; this hook only routes events to it and
 * writes the transform straight to the element (no React render per pointer move).
 */
import { useEffect, useRef, type RefObject } from 'react'
import type { Point, SwipeAxis, SwipeOutcome, ZoomTransform } from './zoomGeometry'
import {
  IDENTITY,
  clampTransform,
  doubleTapScale,
  dragTransform,
  isZoomed,
  lerp,
  lockAxis,
  pinchScale,
  resolveSwipe,
  wheelScale,
  zoomAround,
} from './zoomGeometry'
import { runProgress, type Progress } from './viewerMotion'

export interface SwipeOffset {
  axis: SwipeAxis
  dx: number
  dy: number
}

export interface StageGestureHandlers {
  /** The zoomable element of the current slide; null for a video (no zoom). */
  getMedia(): HTMLElement | null
  onSwipeMove(offset: SwipeOffset): void
  onSwipeEnd(outcome: SwipeOutcome, offset: SwipeOffset): void
  onBackdropTap(): void
  onZoomChange(zoomed: boolean): void
  reduced: boolean
}

export interface StageGestures {
  /** Back to fitted scale instantly (the current item changed). */
  reset(): void
}

const DOUBLE_TAP_MS = 300
const DOUBLE_TAP_DISTANCE = 30
/** Controls that own their own pointer handling (plyr's bar, the viewer chrome). */
const IGNORE_SELECTOR = '.plyr__controls, [data-mv-chrome]'

type Mode = 'idle' | 'pending' | 'pan' | 'pinch' | 'swipe'

const distance = (a: Point, b: Point) => Math.hypot(a.x - b.x, a.y - b.y)
const midpoint = (a: Point, b: Point): Point => ({ x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 })

export function writeTransform(element: HTMLElement | null, t: ZoomTransform): void {
  if (element) element.style.transform = `translate3d(${t.x}px, ${t.y}px, 0) scale(${t.scale})`
}

export function useStageGestures(
  stageRef: RefObject<HTMLElement | null>,
  handlers: StageGestureHandlers,
): StageGestures {
  const latest = useRef(handlers)
  latest.current = handlers
  const transform = useRef<ZoomTransform>(IDENTITY)
  const settling = useRef<Progress | null>(null)
  const zoomedRef = useRef(false)

  useEffect(() => {
    const stage = stageRef.current
    if (!stage) return
    const h = () => latest.current
    const pointers = new Map<number, Point>()
    let mode: Mode = 'idle'
    let start: Point = { x: 0, y: 0 }
    let startT: ZoomTransform = IDENTITY
    let pinch = { distance: 0, mid: { x: 0, y: 0 } }
    let swipe: SwipeOffset = { axis: 'x', dx: 0, dy: 0 }
    let samples: Array<{ p: Point; at: number }> = []
    let lastTap: { p: Point; at: number } | null = null
    let suppressClick = false
    let downTarget: EventTarget | null = null

    const viewport = () => ({ width: stage.clientWidth, height: stage.clientHeight })
    const base = (media: HTMLElement) => ({ width: media.offsetWidth, height: media.offsetHeight })
    const centreOffset = (p: Point): Point => {
      const box = stage.getBoundingClientRect()
      return { x: p.x - box.left - box.width / 2, y: p.y - box.top - box.height / 2 }
    }
    const apply = (t: ZoomTransform) => {
      transform.current = t
      writeTransform(h().getMedia(), t)
      const zoomed = isZoomed(t)
      if (zoomed !== zoomedRef.current) {
        zoomedRef.current = zoomed
        h().onZoomChange(zoomed)
      }
    }
    const stopSettle = () => {
      settling.current?.stop()
      settling.current = null
    }
    const settle = (target: ZoomTransform) => {
      stopSettle()
      const from = transform.current
      settling.current = runProgress(
        (p) =>
          apply({
            scale: lerp(from.scale, target.scale, p),
            x: lerp(from.x, target.x, p),
            y: lerp(from.y, target.y, p),
          }),
        { reduced: h().reduced, from: stage },
      )
    }
    const settleInBounds = () => {
      const media = h().getMedia()
      if (media) settle(clampTransform(transform.current, base(media), viewport()))
    }

    // Capture only once a drag is recognised: capturing on press would retarget the click
    // of a plain tap to the stage and swallow plyr's click-to-play.
    const captureAll = () => {
      for (const id of pointers.keys()) {
        try {
          stage.setPointerCapture(id)
        } catch {
          // The pointer is already gone.
        }
      }
    }

    const beginSingle = (p: Point) => {
      mode = 'pending'
      start = p
      startT = transform.current
      samples = [{ p, at: performance.now() }]
    }

    const onPointerDown = (event: PointerEvent) => {
      if (event.pointerType === 'mouse' && event.button !== 0) return
      if (event.target instanceof Element && event.target.closest(IGNORE_SELECTOR)) return
      if (pointers.size === 0) downTarget = event.target
      stopSettle()
      pointers.set(event.pointerId, { x: event.clientX, y: event.clientY })
      const [a, b] = [...pointers.values()]
      if (a && b && h().getMedia() && mode !== 'swipe') {
        mode = 'pinch'
        startT = transform.current
        pinch = { distance: distance(a, b), mid: midpoint(a, b) }
        captureAll()
      } else if (pointers.size === 1) {
        beginSingle({ x: event.clientX, y: event.clientY })
      }
    }

    const onPointerMove = (event: PointerEvent) => {
      if (!pointers.has(event.pointerId)) return
      const p = { x: event.clientX, y: event.clientY }
      pointers.set(event.pointerId, p)
      samples = [...samples.slice(-4), { p, at: performance.now() }]
      const media = h().getMedia()
      if (mode === 'pinch' && media) {
        const [a, b] = [...pointers.values()]
        if (!a || !b) return
        const mid = midpoint(a, b)
        const zoomed = zoomAround(
          startT,
          pinchScale(startT.scale, pinch.distance, distance(a, b)),
          centreOffset(pinch.mid),
        )
        apply({ ...zoomed, x: zoomed.x + mid.x - pinch.mid.x, y: zoomed.y + mid.y - pinch.mid.y })
        return
      }
      const delta = { x: p.x - start.x, y: p.y - start.y }
      if (mode === 'pending') {
        if (media && isZoomed(startT)) {
          if (Math.hypot(delta.x, delta.y) > 3) mode = 'pan'
        } else {
          const axis = lockAxis(delta.x, delta.y)
          if (axis) {
            mode = 'swipe'
            swipe = { axis, dx: 0, dy: 0 }
          }
        }
        if (mode !== 'pending') captureAll()
      }
      if (mode === 'pan' && media) apply(dragTransform(startT, delta, base(media), viewport()))
      if (mode === 'swipe') {
        swipe = { axis: swipe.axis, dx: swipe.axis === 'x' ? delta.x : 0, dy: swipe.axis === 'y' ? delta.y : 0 }
        h().onSwipeMove(swipe)
      }
    }

    const velocity = (): Point => {
      const first = samples[0]
      const last = samples.at(-1)
      if (!first || !last || last.at - first.at < 1) return { x: 0, y: 0 }
      return { x: (last.p.x - first.p.x) / (last.at - first.at), y: (last.p.y - first.p.y) / (last.at - first.at) }
    }

    const onTap = (event: PointerEvent) => {
      const media = h().getMedia()
      const p = { x: event.clientX, y: event.clientY }
      // Pointer capture retargets `pointerup` to the stage; the press target is the truth.
      const target = downTarget
      if (!media || !(target instanceof Node) || !media.contains(target)) {
        // A video's own surface is not "backdrop": plyr toggles playback on it.
        if (target instanceof Element && target.closest('.plyr, [data-mv-video], .tg-mv__still')) return
        h().onBackdropTap()
        return
      }
      const now = performance.now()
      if (lastTap && now - lastTap.at < DOUBLE_TAP_MS && distance(lastTap.p, p) < DOUBLE_TAP_DISTANCE) {
        lastTap = null
        const target = zoomAround(transform.current, doubleTapScale(transform.current.scale), centreOffset(p))
        settle(clampTransform(target, base(media), viewport()))
        return
      }
      lastTap = { p, at: now }
    }

    const onPointerUp = (event: PointerEvent) => {
      if (!pointers.delete(event.pointerId)) return
      const ended = mode
      if (ended === 'pinch') {
        const [rest] = [...pointers.values()]
        if (rest) {
          beginSingle(rest)
          mode = 'pan'
        } else {
          mode = 'idle'
          settleInBounds()
        }
        suppressClick = true
        return
      }
      if (pointers.size > 0) return
      mode = 'idle'
      if (ended === 'pending' && event.type === 'pointerup') onTap(event)
      if (ended === 'pan') settleInBounds()
      if (ended === 'swipe') {
        const v = velocity()
        h().onSwipeEnd(resolveSwipe({ ...swipe, vx: v.x, vy: v.y, viewport: viewport() }), swipe)
      }
      if (ended === 'pan' || ended === 'swipe') suppressClick = true
    }

    const onClickCapture = (event: MouseEvent) => {
      if (!suppressClick) return
      suppressClick = false
      event.stopPropagation()
      event.preventDefault()
    }

    const onWheel = (event: WheelEvent) => {
      const media = h().getMedia()
      if (!media || (event.target instanceof Element && event.target.closest(IGNORE_SELECTOR))) return
      event.preventDefault()
      stopSettle()
      const t = transform.current
      const next = zoomAround(
        t,
        wheelScale(t.scale, event.deltaY, event.deltaMode, event.ctrlKey),
        centreOffset({ x: event.clientX, y: event.clientY }),
      )
      apply(clampTransform(next, base(media), viewport()))
    }

    stage.addEventListener('pointerdown', onPointerDown)
    stage.addEventListener('pointermove', onPointerMove)
    stage.addEventListener('pointerup', onPointerUp)
    stage.addEventListener('pointercancel', onPointerUp)
    stage.addEventListener('click', onClickCapture, true)
    stage.addEventListener('wheel', onWheel, { passive: false })
    return () => {
      stopSettle()
      stage.removeEventListener('pointerdown', onPointerDown)
      stage.removeEventListener('pointermove', onPointerMove)
      stage.removeEventListener('pointerup', onPointerUp)
      stage.removeEventListener('pointercancel', onPointerUp)
      stage.removeEventListener('click', onClickCapture, true)
      stage.removeEventListener('wheel', onWheel)
    }
  }, [stageRef])

  return {
    reset() {
      settling.current?.stop()
      settling.current = null
      transform.current = IDENTITY
      writeTransform(latest.current.getMedia(), IDENTITY)
      if (zoomedRef.current) latest.current.onZoomChange(false)
      zoomedRef.current = false
    },
  }
}

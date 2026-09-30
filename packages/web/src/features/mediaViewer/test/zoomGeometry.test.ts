import { describe, expect, test } from 'bun:test'
import {
  DOUBLE_TAP_SCALE,
  IDENTITY,
  MAX_SCALE,
  centreIn,
  clampTransform,
  dismissOpacity,
  doubleTapScale,
  dragTransform,
  fitContain,
  lockAxis,
  panLimits,
  pinchScale,
  resolveSwipe,
  rubberBand,
  wheelScale,
  zoomAround,
} from '../zoomGeometry'

const viewport = { width: 1000, height: 800 }

describe('fit', () => {
  test('contains without upscaling', () => {
    expect(fitContain({ width: 4000, height: 2000 }, viewport)).toEqual({ width: 1000, height: 500 })
    expect(fitContain({ width: 1000, height: 4000 }, viewport)).toEqual({ width: 200, height: 800 })
    expect(fitContain({ width: 300, height: 200 }, viewport)).toEqual({ width: 300, height: 200 })
    expect(fitContain({ width: 0, height: 10 }, viewport)).toEqual({ width: 0, height: 0 })
  })

  test('centres inside a box with an origin', () => {
    expect(centreIn({ width: 100, height: 50 }, { x: 10, y: 20, width: 300, height: 150 })).toEqual({
      x: 110,
      y: 70,
      width: 100,
      height: 50,
    })
  })
})

describe('pan bounds', () => {
  const base = { width: 1000, height: 500 }

  test('no travel until the medium overflows the viewport', () => {
    expect(panLimits(1, base, viewport)).toEqual({ x: 0, y: 0 })
    // 1.2 × 500 = 600 < 800: still no vertical travel, 200 px of horizontal overflow.
    expect(panLimits(1.2, base, viewport)).toEqual({ x: 100, y: 0 })
    expect(panLimits(2, base, viewport)).toEqual({ x: 500, y: 100 })
  })

  test('clamping keeps every edge covered and the scale in range', () => {
    expect(clampTransform({ scale: 2, x: 900, y: -900 }, base, viewport)).toEqual({ scale: 2, x: 500, y: -100 })
    expect(clampTransform({ scale: 0.5, x: 40, y: 40 }, base, viewport)).toEqual(IDENTITY)
    expect(clampTransform({ scale: 9, x: 0, y: 0 }, base, viewport).scale).toBe(MAX_SCALE)
  })

  test('inside the bounds a drag is free; beyond them it resists but still moves', () => {
    const start = { scale: 2, x: 0, y: 0 }
    expect(dragTransform(start, { x: 300, y: -50 }, base, viewport)).toEqual({ scale: 2, x: 300, y: -50 })
    const over = dragTransform(start, { x: 900, y: 0 }, base, viewport)
    expect(over.x).toBeGreaterThan(500)
    expect(over.x).toBeLessThan(900)
  })

  test('rubber band is identity inside, monotonic and bounded outside', () => {
    expect(rubberBand(50, -100, 100, 1000)).toBe(50)
    const a = rubberBand(150, -100, 100, 1000)
    const b = rubberBand(400, -100, 100, 1000)
    expect(a).toBeGreaterThan(100)
    expect(b).toBeGreaterThan(a)
    expect(rubberBand(1e9, -100, 100, 1000)).toBeLessThan(1100)
    expect(rubberBand(-150, -100, 100, 1000)).toBeCloseTo(-a, 6)
  })
})

describe('zoom', () => {
  test('zooming keeps the focused content point fixed on screen', () => {
    const t = { scale: 1.5, x: 40, y: -20 }
    const focus = { x: 200, y: 100 }
    const next = zoomAround(t, 3, focus)
    // Content point under focus before: (focus - t) / scale; after it must map back to focus.
    const content = { x: (focus.x - t.x) / t.scale, y: (focus.y - t.y) / t.scale }
    expect(content.x * next.scale + next.x).toBeCloseTo(focus.x, 9)
    expect(content.y * next.scale + next.y).toBeCloseTo(focus.y, 9)
  })

  test('wheel: down zooms out, up zooms in, trackpad pinch is amplified, limits hold', () => {
    expect(wheelScale(2, 100, 0, false)).toBeLessThan(2)
    expect(wheelScale(2, -100, 0, false)).toBeGreaterThan(2)
    expect(wheelScale(2, -10, 0, true)).toBeGreaterThan(wheelScale(2, -10, 0, false))
    expect(wheelScale(1, 500, 0, false)).toBe(1)
    expect(wheelScale(MAX_SCALE, -500, 0, false)).toBe(MAX_SCALE)
    // Line mode is converted to pixels.
    expect(wheelScale(2, -3, 1, false)).toBeCloseTo(wheelScale(2, -48, 0, false), 9)
  })

  test('pinch follows the finger ratio within soft limits', () => {
    expect(pinchScale(1, 100, 200)).toBe(2)
    expect(pinchScale(2, 100, 50)).toBe(1)
    expect(pinchScale(1, 100, 10)).toBeGreaterThan(0.5)
    expect(pinchScale(4, 100, 1000)).toBeLessThan(MAX_SCALE * 1.5)
    expect(pinchScale(1.7, 0, 50)).toBe(1.7)
  })

  test('double tap toggles fitted ↔ close-up', () => {
    expect(doubleTapScale(1)).toBe(DOUBLE_TAP_SCALE)
    expect(doubleTapScale(1.8)).toBe(1)
  })
})

describe('swipes', () => {
  test('axis locks only past the slop, by the dominant direction', () => {
    expect(lockAxis(3, 4)).toBeNull()
    expect(lockAxis(20, 5)).toBe('x')
    expect(lockAxis(-4, -30)).toBe('y')
  })

  const swipe = (over: Partial<Parameters<typeof resolveSwipe>[0]>) =>
    resolveSwipe({ axis: 'x', dx: 0, dy: 0, vx: 0, vy: 0, viewport, ...over })

  test('horizontal: distance or a flick in the same direction flips; dragging left goes newer', () => {
    expect(swipe({ dx: -300 })).toBe('next')
    expect(swipe({ dx: 300 })).toBe('prev')
    expect(swipe({ dx: -60 })).toBe('cancel')
    expect(swipe({ dx: -60, vx: -0.9 })).toBe('next')
    // A flick back against the drag direction cancels.
    expect(swipe({ dx: -60, vx: 0.9 })).toBe('cancel')
  })

  test('vertical: either direction dismisses past 15% or with a flick', () => {
    expect(swipe({ axis: 'y', dy: 200 })).toBe('dismiss')
    expect(swipe({ axis: 'y', dy: -200 })).toBe('dismiss')
    expect(swipe({ axis: 'y', dy: 40 })).toBe('cancel')
    expect(swipe({ axis: 'y', dy: 40, vy: 1 })).toBe('dismiss')
  })

  test('backdrop dims with the dismiss drag, never below 20%', () => {
    expect(dismissOpacity(0, 800)).toBe(1)
    expect(dismissOpacity(200, 800)).toBeCloseTo(0.6, 9)
    expect(dismissOpacity(-5000, 800)).toBeCloseTo(0.2, 9)
  })
})

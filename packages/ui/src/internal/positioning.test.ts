import { expect, test } from 'bun:test'
import { formatPlacement, parsePlacement, placeFloating, pointRect, type PlaceInput } from './positioning'

const input = (over: Partial<PlaceInput> = {}): PlaceInput => ({
  anchor: { x: 200, y: 200, width: 100, height: 40 },
  floating: { width: 160, height: 120 },
  viewport: { width: 1000, height: 800 },
  placement: 'bottom-start',
  offset: 8,
  padding: 8,
  ...over,
})

test('placement parsing and formatting round-trip', () => {
  expect(parsePlacement('bottom')).toEqual({ side: 'bottom', align: 'center' })
  expect(parsePlacement('top-end')).toEqual({ side: 'top', align: 'end' })
  expect(formatPlacement('left', 'center')).toBe('left')
  expect(formatPlacement('left', 'start')).toBe('left-start')
})

test('bottom-start sits under the anchor, aligned to its leading edge', () => {
  expect(placeFloating(input())).toEqual({ x: 200, y: 248, placement: 'bottom-start' })
})

test('the three alignments differ only on the cross axis', () => {
  expect(placeFloating(input({ placement: 'bottom' })).x).toBe(170)
  expect(placeFloating(input({ placement: 'bottom-end' })).x).toBe(140)
  expect(placeFloating(input({ placement: 'bottom-start' })).x).toBe(200)
})

test('top places the overlay above the anchor', () => {
  expect(placeFloating(input({ placement: 'top-start' }))).toEqual({ x: 200, y: 72, placement: 'top-start' })
})

test('left and right place on the inline axis and align on the block axis', () => {
  expect(placeFloating(input({ placement: 'right-start' }))).toEqual({ x: 308, y: 200, placement: 'right-start' })
  expect(placeFloating(input({ placement: 'left-start' }))).toEqual({ x: 32, y: 200, placement: 'left-start' })
})

test('a side that does not fit flips to the opposite side, and the result reports it', () => {
  const tight = input({ anchor: { x: 200, y: 700, width: 100, height: 40 }, placement: 'bottom-start' })
  expect(placeFloating(tight)).toEqual({ x: 200, y: 572, placement: 'top-start' })
})

test('it does NOT flip when the opposite side does not fit either', () => {
  const squeezed = input({
    anchor: { x: 200, y: 300, width: 100, height: 40 },
    floating: { width: 160, height: 400 },
    viewport: { width: 1000, height: 500 },
    placement: 'bottom-start',
  })
  // Neither side fits, so the requested side is kept and the clamp does the rest.
  expect(placeFloating(squeezed).placement).toBe('bottom-start')
  expect(placeFloating(squeezed).y).toBe(92)
})

test('the cross axis is clamped inside the viewport padding', () => {
  const nearEdge = input({ anchor: { x: 960, y: 200, width: 30, height: 40 }, placement: 'bottom-start' })
  expect(placeFloating(nearEdge).x).toBe(832)

  const nearStart = input({ anchor: { x: 2, y: 200, width: 30, height: 40 }, placement: 'bottom-end' })
  expect(placeFloating(nearStart).x).toBe(8)
})

test('an overlay larger than the viewport is pinned to the leading edge, not given a negative offset', () => {
  const huge = input({ floating: { width: 2000, height: 2000 }, viewport: { width: 400, height: 400 } })
  expect(placeFloating(huge)).toMatchObject({ x: 8, y: 8 })
})

test('a pointer anchor is a zero-size rect, so a context menu opens at the click point', () => {
  const at = placeFloating(input({ anchor: pointRect(120, 90), placement: 'bottom-start' }))
  expect(at).toEqual({ x: 120, y: 98, placement: 'bottom-start' })
})

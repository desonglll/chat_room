import { expect, test } from 'bun:test'
import { hostCentre, rippleGeometry } from './rippleGeometry'

const host = { width: 200, height: 48 }

test('the wave is centred on the pointer, not on the element', () => {
  const near = rippleGeometry({ host, pointer: { x: 10, y: 24 } })
  const far = rippleGeometry({ host, pointer: { x: 190, y: 24 } })

  // The only difference between a click at either end is where the circle starts.
  expect(near.left).toBe(10 - 100)
  expect(far.left).toBe(190 - 100)
  expect(near.diameter).toBe(far.diameter)
})

test('the diameter is the larger side of the host, which is Telegram and not Material', () => {
  expect(rippleGeometry({ host, pointer: { x: 0, y: 0 } }).diameter).toBe(200)
  expect(rippleGeometry({ host: { width: 40, height: 96 }, pointer: { x: 0, y: 0 } }).diameter).toBe(96)

  // Material sizes from the diagonal so the wave is guaranteed to cover the farthest corner.
  // Telegram does not, and this asserts the shortfall rather than quietly "fixing" it: at the
  // token end scale of 2 the reach is `diameter`, while the farthest corner of this host from a
  // corner click is sqrt(200^2 + 48^2) = 205.7px away.
  const corner = rippleGeometry({ host, pointer: { x: 0, y: 0 } })
  const reach = corner.diameter
  const farthestCorner = Math.hypot(host.width, host.height)
  expect(reach).toBeLessThan(farthestCorner)
})

test('the geometry does not move between start and end, so there is no centre migration', () => {
  // Material translates the circle from the pointer toward the element centre while it grows
  // (`--mdc-ripple-fg-translate-start` / `-end`). There is no translation to express here at all:
  // the only animated property is `scale`, which is why this function returns one position.
  const geometry = rippleGeometry({ host, pointer: { x: 25, y: 10 } })
  expect(Object.keys(geometry).sort()).toEqual(['diameter', 'left', 'top'])
  expect(geometry.top).toBe(10 - 100)
})

test('keyboard activation falls back to the host centre', () => {
  expect(hostCentre(host)).toEqual({ x: 100, y: 24 })
  const geometry = rippleGeometry({ host, pointer: hostCentre(host) })
  expect(geometry.left).toBe(0)
  expect(geometry.top).toBe(24 - 100)
})

test('a zero-size host produces a zero wave instead of NaN', () => {
  expect(rippleGeometry({ host: { width: 0, height: 0 }, pointer: { x: 0, y: 0 } })).toEqual({
    left: 0,
    top: 0,
    diameter: 0,
  })
})

import { describe, expect, test } from 'bun:test'
import { nextFocusIndex, typeaheadIndex } from './rovingFocus'

const base = { count: 4 } as const

describe('nextFocusIndex', () => {
  test('vertical orientation moves on ArrowDown/ArrowUp and ignores horizontal keys', () => {
    expect(nextFocusIndex({ ...base, key: 'ArrowDown', current: 0 })).toEqual({ index: 1, handled: true })
    expect(nextFocusIndex({ ...base, key: 'ArrowUp', current: 2 })).toEqual({ index: 1, handled: true })
    expect(nextFocusIndex({ ...base, key: 'ArrowRight', current: 1 })).toEqual({ index: 1, handled: false })
  })

  test('horizontal orientation moves on ArrowRight/ArrowLeft only', () => {
    const o = { ...base, orientation: 'horizontal' } as const
    expect(nextFocusIndex({ ...o, key: 'ArrowRight', current: 0 })).toEqual({ index: 1, handled: true })
    expect(nextFocusIndex({ ...o, key: 'ArrowLeft', current: 3 })).toEqual({ index: 2, handled: true })
    expect(nextFocusIndex({ ...o, key: 'ArrowDown', current: 0 }).handled).toBe(false)
  })

  test("'both' accepts all four arrows", () => {
    const o = { ...base, orientation: 'both' } as const
    for (const key of ['ArrowDown', 'ArrowRight']) {
      expect(nextFocusIndex({ ...o, key, current: 0 })).toEqual({ index: 1, handled: true })
    }
    for (const key of ['ArrowUp', 'ArrowLeft']) {
      expect(nextFocusIndex({ ...o, key, current: 1 })).toEqual({ index: 0, handled: true })
    }
  })

  test('wraps by default and stops at the ends when loop is false', () => {
    expect(nextFocusIndex({ ...base, key: 'ArrowDown', current: 3 }).index).toBe(0)
    expect(nextFocusIndex({ ...base, key: 'ArrowUp', current: 0 }).index).toBe(3)
    expect(nextFocusIndex({ ...base, key: 'ArrowDown', current: 3, loop: false }).index).toBe(3)
    expect(nextFocusIndex({ ...base, key: 'ArrowUp', current: 0, loop: false }).index).toBe(0)
  })

  test('skips disabled entries in both directions and when wrapping', () => {
    const isDisabled = (index: number) => index === 1 || index === 2
    expect(nextFocusIndex({ ...base, key: 'ArrowDown', current: 0, isDisabled }).index).toBe(3)
    expect(nextFocusIndex({ ...base, key: 'ArrowUp', current: 3, isDisabled }).index).toBe(0)
    expect(nextFocusIndex({ ...base, key: 'ArrowDown', current: 3, isDisabled }).index).toBe(0)
  })

  test('Home and End land on the first and last ENABLED entry', () => {
    const isDisabled = (index: number) => index === 0 || index === 3
    expect(nextFocusIndex({ ...base, key: 'Home', current: 2, isDisabled })).toEqual({ index: 1, handled: true })
    expect(nextFocusIndex({ ...base, key: 'End', current: 1, isDisabled })).toEqual({ index: 2, handled: true })
  })

  test('from "nothing focused" a forward key lands on the first entry, not the second', () => {
    expect(nextFocusIndex({ ...base, key: 'ArrowDown', current: -1 }).index).toBe(0)
    expect(nextFocusIndex({ ...base, key: 'ArrowUp', current: -1 }).index).toBe(3)
  })

  test('a set where everything is disabled terminates instead of spinning', () => {
    const result = nextFocusIndex({ ...base, key: 'ArrowDown', current: 0, isDisabled: () => true })
    expect(result).toEqual({ index: 0, handled: true })
  })

  test('an empty set handles nothing', () => {
    expect(nextFocusIndex({ key: 'ArrowDown', current: -1, count: 0 }).handled).toBe(false)
    expect(nextFocusIndex({ key: 'Home', current: -1, count: 0 }).handled).toBe(false)
  })

  test('rtl mirrors the horizontal arrows and leaves the vertical ones alone', () => {
    const o = { ...base, orientation: 'horizontal', rtl: true } as const
    expect(nextFocusIndex({ ...o, key: 'ArrowLeft', current: 0 }).index).toBe(1)
    expect(nextFocusIndex({ ...o, key: 'ArrowRight', current: 1 }).index).toBe(0)
    expect(nextFocusIndex({ ...base, rtl: true, key: 'ArrowDown', current: 0 }).index).toBe(1)
  })
})

describe('typeaheadIndex', () => {
  const labels = ['Reply', 'Forward', 'Copy', 'Report', 'Delete']

  test('finds the next match after the current index, cycling', () => {
    expect(typeaheadIndex(labels, 'r', -1)).toBe(0)
    expect(typeaheadIndex(labels, 'r', 0)).toBe(3)
    expect(typeaheadIndex(labels, 'r', 3)).toBe(0)
  })

  test('is case insensitive and returns -1 when nothing matches', () => {
    expect(typeaheadIndex(labels, 'D', -1)).toBe(4)
    expect(typeaheadIndex(labels, 'z', -1)).toBe(-1)
    expect(typeaheadIndex(labels, '', -1)).toBe(-1)
  })
})

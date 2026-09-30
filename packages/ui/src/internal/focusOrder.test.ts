import { expect, test } from 'bun:test'
import { isTabbable, nextTabStop, tabbableIndexes, type FocusCandidate } from './focusOrder'

const stop = (over: Partial<FocusCandidate> = {}): FocusCandidate => ({
  tabIndex: 0,
  disabled: false,
  hidden: false,
  ...over,
})

test('a candidate is tabbable only when enabled, visible and non-negative', () => {
  expect(isTabbable(stop())).toBe(true)
  expect(isTabbable(stop({ disabled: true }))).toBe(false)
  expect(isTabbable(stop({ hidden: true }))).toBe(false)
  expect(isTabbable(stop({ tabIndex: -1 }))).toBe(false)
  expect(isTabbable(stop({ tabIndex: 3 }))).toBe(true)
})

test('tabbableIndexes returns positions in the original array', () => {
  const list = [stop({ disabled: true }), stop(), stop({ tabIndex: -1 }), stop()]
  expect(tabbableIndexes(list)).toEqual([1, 3])
})

test('Tab cycles forwards and wraps at the last stop', () => {
  const list = [stop(), stop(), stop()]
  expect(nextTabStop(list, 0, false)).toBe(1)
  expect(nextTabStop(list, 1, false)).toBe(2)
  expect(nextTabStop(list, 2, false)).toBe(0)
})

test('Shift+Tab cycles backwards and wraps at the first stop', () => {
  const list = [stop(), stop(), stop()]
  expect(nextTabStop(list, 2, true)).toBe(1)
  expect(nextTabStop(list, 0, true)).toBe(2)
})

test('untabbable entries are skipped in both directions', () => {
  const list = [stop(), stop({ disabled: true }), stop({ hidden: true }), stop()]
  expect(nextTabStop(list, 0, false)).toBe(3)
  expect(nextTabStop(list, 3, true)).toBe(0)
})

test('focus on the container (current = -1) enters at the correct end', () => {
  const list = [stop({ tabIndex: -1 }), stop(), stop()]
  expect(nextTabStop(list, -1, false)).toBe(1)
  expect(nextTabStop(list, -1, true)).toBe(2)
})

test('a set with no tab stop returns -1, which tells the caller to hold the container', () => {
  expect(nextTabStop([], 0, false)).toBe(-1)
  expect(nextTabStop([stop({ disabled: true })], 0, false)).toBe(-1)
})

test('a single tab stop stays on itself rather than escaping the trap', () => {
  const list = [stop()]
  expect(nextTabStop(list, 0, false)).toBe(0)
  expect(nextTabStop(list, 0, true)).toBe(0)
})

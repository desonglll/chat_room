import { expect, test } from 'bun:test'
import {
  OLDER_PAGE_THRESHOLD,
  indexOfItem,
  neighbourId,
  positionLabel,
  shouldLoadOlder,
  stripWindow,
  successorAfterRemoval,
} from '../mediaNavigation'
import { media } from './fixtures'

const items = [media(1), media(2), media(3)]

test('neighbours: left is older, right is newer, null at the ends and for unknown ids', () => {
  expect(indexOfItem(items, 'a2')).toBe(1)
  expect(neighbourId(items, 'a2', -1)).toBe('a1')
  expect(neighbourId(items, 'a2', 1)).toBe('a3')
  expect(neighbourId(items, 'a1', -1)).toBeNull()
  expect(neighbourId(items, 'a3', 1)).toBeNull()
  expect(neighbourId(items, 'zz', 1)).toBeNull()
})

test('after a delete the newer neighbour wins, then the older, then nothing', () => {
  expect(successorAfterRemoval(items, 'a2')).toBe('a3')
  expect(successorAfterRemoval(items, 'a3')).toBe('a2')
  expect(successorAfterRemoval([media(1)], 'a1')).toBeNull()
})

test('older pages are requested near the oldest end only while more exist', () => {
  expect(shouldLoadOlder(0, true)).toBe(true)
  expect(shouldLoadOlder(OLDER_PAGE_THRESHOLD - 1, true)).toBe(true)
  expect(shouldLoadOlder(OLDER_PAGE_THRESHOLD, true)).toBe(false)
  expect(shouldLoadOlder(0, false)).toBe(false)
  expect(shouldLoadOlder(-1, true)).toBe(false)
})

test('strip window stays full and inside the list', () => {
  expect(stripWindow(100, 50, 5)).toEqual({ start: 45, end: 56 })
  expect(stripWindow(100, 1, 5)).toEqual({ start: 0, end: 11 })
  expect(stripWindow(100, 99, 5)).toEqual({ start: 89, end: 100 })
  expect(stripWindow(4, 2, 5)).toEqual({ start: 0, end: 4 })
  expect(stripWindow(0, 0, 5)).toEqual({ start: 0, end: 0 })
})

test('position label only when the whole list is known', () => {
  expect(positionLabel(2, 10, true)).toBe('3 / 10')
  expect(positionLabel(2, 10, false)).toBe('')
  expect(positionLabel(-1, 10, true)).toBe('')
})

/** TG-805: only rows present in both layouts and actually moved get a FLIP delta. */
import { expect, test } from 'bun:test'
import { flipDeltas } from './flipReorder'

test('a chat jumping to the top moves it up and the rows it passed down', () => {
  const before = new Map([
    ['a', 0],
    ['b', 72],
    ['c', 144],
  ])
  const after = new Map([
    ['c', 0],
    ['a', 72],
    ['b', 144],
  ])
  expect(flipDeltas(before, after)).toEqual([
    ['c', 144],
    ['a', -72],
    ['b', -72],
  ])
})

test('new rows and unmoved rows are left alone; sub-pixel jitter is ignored', () => {
  const before = new Map([
    ['a', 72],
    ['b', 144],
  ])
  const after = new Map([
    ['n', 0],
    ['a', 72.4],
    ['b', 216],
  ])
  expect(flipDeltas(before, after)).toEqual([['b', -72]])
  expect(flipDeltas(new Map(), after)).toEqual([])
})

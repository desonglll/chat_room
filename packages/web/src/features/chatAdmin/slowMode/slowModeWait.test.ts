// TG-801: «慢速模式已开启» blocked sending in chats without slow mode. `now` was read once at
// mount, the loaded deadline (`Date.now() + 0`) landed after it, and the ticker never ran for
// a deadline already in the past — so the stale gap read as seconds left, forever.
import { expect, test } from 'bun:test'
import { slowModeWait } from './useSlowMode'

const off = { seconds: 0, wait_seconds: 0, exempt: false }
const on = { seconds: 30, wait_seconds: 0, exempt: false }

test('a chat without slow mode never waits, even with a stale clock', () => {
  expect(slowModeWait(off, 1_000_000, 1_000_000 - 120_000)).toBe(0)
})

test('an exempt viewer never waits', () => {
  expect(slowModeWait({ ...on, exempt: true }, 1_030_000, 1_000_000)).toBe(0)
})

test('slow mode counts down to the deadline in whole seconds', () => {
  expect(slowModeWait(on, 1_030_000, 1_000_000)).toBe(30)
  expect(slowModeWait(on, 1_030_000, 1_029_001)).toBe(1)
  expect(slowModeWait(on, 1_030_000, 1_031_000)).toBe(0)
})

test('nothing is loaded yet: no wait', () => {
  expect(slowModeWait(null, 5_000, 0)).toBe(0)
})

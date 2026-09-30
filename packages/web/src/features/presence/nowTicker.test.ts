// TG-107: the shared ticker — one timer, period ticks, exact wake-ups, idle when unobserved.
import { expect, test } from 'bun:test'
import { createNowTicker } from './nowTicker'
import { FakeClock } from './testClock'

test('one timer serves every subscriber and stops when the last leaves', () => {
  const clock = new FakeClock(1_000)
  const ticker = createNowTicker({ clock, periodMs: 5_000 })
  let a = 0
  let b = 0
  const offA = ticker.subscribe(() => a++)
  const offB = ticker.subscribe(() => b++)
  expect(clock.pending()).toBe(1)
  clock.advance(10_000)
  expect([a, b]).toEqual([2, 2])
  expect(ticker.now()).toBe(11_000)
  offA()
  offB()
  expect(clock.pending()).toBe(0)
  clock.advance(1)
  expect(ticker.now()).toBe(11_001) // idle: live read
})

test('wakes exactly at the wake source instant, and reschedules when the source changes', () => {
  const clock = new FakeClock(0)
  let wakeAt: number | null = null
  let notifySource: () => void = () => {}
  const ticker = createNowTicker({
    clock,
    periodMs: 5_000,
    wakeSource: {
      subscribe: (listener) => {
        notifySource = listener
        return () => {
          notifySource = () => {}
        }
      },
      nextWakeAt: (now) => (wakeAt !== null && wakeAt > now ? wakeAt : null),
    },
  })
  const ticks: number[] = []
  ticker.subscribe(() => ticks.push(ticker.now()))
  wakeAt = 1_234
  notifySource()
  expect(clock.pending()).toBe(1)
  clock.advance(6_300)
  expect(ticks).toEqual([1_234, 6_234])
})

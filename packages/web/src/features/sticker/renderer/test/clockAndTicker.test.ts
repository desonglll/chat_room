import { describe, expect, test } from 'bun:test'
import { createFrameTicker, type TickPlayer, type TickerHost } from '../frameTicker'
import { MAX_STEP_MS, advanceClock, createClock, frameToDraw, restartClock } from '../playerClock'

describe('playerClock', () => {
  test('advances by elapsed time at the animation frame rate, and loops', () => {
    const clock = createClock(180, 60, true)
    advanceClock(clock, 50)
    expect(clock.position).toBeCloseTo(3)
    for (let step = 0; step < 60; step += 1) advanceClock(clock, 50)
    expect(clock.position).toBeCloseTo(3)
    expect(clock.ended).toBe(false)
  })

  test('a stall advances by at most MAX_STEP_MS', () => {
    const clock = createClock(180, 60, true)
    advanceClock(clock, 10_000)
    expect(clock.position).toBeCloseTo((MAX_STEP_MS * 60) / 1000)
  })

  test('a play-once clock stops on the last frame and reports ended until restarted', () => {
    const clock = createClock(30, 30, false)
    for (let step = 0; step < 20; step += 1) advanceClock(clock, 100)
    expect(clock.ended).toBe(true)
    expect(frameToDraw(clock, 30)).toBe(29)
    restartClock(clock)
    expect(clock.ended).toBe(false)
    expect(frameToDraw(clock, 30)).toBe(0)
  })

  test('the fps cap draws every n-th frame without slowing the animation down', () => {
    const clock = createClock(180, 60, true)
    advanceClock(clock, 55)
    expect(Math.floor(clock.position)).toBe(3)
    expect(frameToDraw(clock, 30)).toBe(2)
    expect(frameToDraw(clock, 60)).toBe(3)
    expect(frameToDraw(clock, 20)).toBe(3)
  })
})

function fakeHost() {
  let now = 0
  let queued: ((now: number) => void) | null = null
  let handles = 0
  const host: TickerHost & { cost: number } = {
    cost: 0,
    requestFrame(callback) {
      queued = callback
      return ++handles
    },
    cancelFrame() {
      queued = null
    },
    now: () => now,
  }
  const frame = (ms = 16) => {
    now += ms
    const callback = queued
    queued = null
    callback?.(now)
  }
  const spend = (ms: number) => {
    now += ms
  }
  return { host, frame, spend, pending: () => queued !== null }
}

function player(spend: (ms: number) => void, drawCost: number, log: string[], name: string): TickPlayer {
  return {
    advance: () => true,
    draw: () => {
      spend(drawCost)
      log.push(name)
    },
  }
}

describe('frameTicker', () => {
  test('stops requesting frames when nothing plays', () => {
    const { host, frame, pending } = fakeHost()
    const ticker = createFrameTicker(host, { budgetMs: 8, fpsCap: () => 60 })
    const a: TickPlayer = { advance: () => false, draw: () => undefined }
    expect(pending()).toBe(false)
    ticker.add(a)
    expect(pending()).toBe(true)
    frame()
    expect(pending()).toBe(true)
    ticker.remove(a)
    expect(pending()).toBe(false)
    expect(ticker.running).toBe(false)
  })

  test('the budget defers draws round-robin so every player is served', () => {
    const { host, frame, spend } = fakeHost()
    const ticker = createFrameTicker(host, { budgetMs: 8, fpsCap: () => 60 })
    const log: string[] = []
    for (const name of ['a', 'b', 'c', 'd']) ticker.add(player(spend, 5, log, name))
    frame()
    // 5 ms each against an 8 ms budget: two draws per tick.
    expect(log).toEqual(['a', 'b'])
    frame()
    expect(log).toEqual(['a', 'b', 'c', 'd'])
    frame()
    expect(log.slice(4)).toEqual(['a', 'b'])
    expect(ticker.stats().deferred).toBeGreaterThan(0)
  })

  test('the fps cap sees how many players run', () => {
    const { host, frame } = fakeHost()
    const seen: number[] = []
    const ticker = createFrameTicker(host, {
      budgetMs: 8,
      fpsCap: (count) => {
        seen.push(count)
        return 30
      },
    })
    ticker.add({ advance: () => false, draw: () => undefined })
    ticker.add({ advance: () => false, draw: () => undefined })
    frame()
    expect(seen).toEqual([2])
  })
})

test('asking ahead leads by one stride and wraps or clamps at the end', () => {
  const loop = createClock(180, 60, true)
  advanceClock(loop, 55)
  expect(frameToDraw(loop, 30, true)).toBe(4)
  loop.position = 179
  expect(frameToDraw(loop, 30, true)).toBe(0)
  const once = createClock(180, 60, false)
  once.position = 179
  // Clamped to the last frame, then quantised; the ended clock itself returns frame 179.
  expect(frameToDraw(once, 30, true)).toBe(178)
})

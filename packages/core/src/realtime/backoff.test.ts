import { describe, expect, test } from 'bun:test'
import { reconnectDelayMs } from './backoff'

describe('reconnect backoff', () => {
  test('doubles from 500ms and caps at 5s (the frozen Vue client curve)', () => {
    expect([0, 1, 2, 3, 4, 9].map((attempt) => reconnectDelayMs(attempt))).toEqual([
      500, 1_000, 2_000, 4_000, 5_000, 5_000,
    ])
  })

  test('optional jitter stays within ±25% and only applies when a random source is given', () => {
    expect(reconnectDelayMs(1, { random: () => 0 })).toBe(750)
    expect(reconnectDelayMs(1, { random: () => 1 })).toBe(1_250)
    expect(reconnectDelayMs(1, { random: () => 0.5 })).toBe(1_000)
  })
})

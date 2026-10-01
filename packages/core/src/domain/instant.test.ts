import { expect, test } from 'bun:test'
import { compareInstants } from './instant'

test('instants compare by time, not by string', () => {
  // Lexically '.' (0x2E) < 'Z' (0x5A), so the string order is backwards here.
  expect(compareInstants('2026-10-01T09:00:00.500Z', '2026-10-01T09:00:00Z')).toBeGreaterThan(0)
  expect('2026-10-01T09:00:00.500Z' > '2026-10-01T09:00:00Z').toBe(false)
  expect(compareInstants('2026-10-01T17:00:00+08:00', '2026-10-01T09:00:00Z')).toBe(0)
  expect(compareInstants('garbage', '2026-10-01T09:00:00Z')).toBeLessThan(0)
})

/**
 * The TG-101 benchmark as a regression guard: same code as the budget script, 5x slack, so
 * only an algorithmic regression (not a busy CI box) fails `bun test`.
 */
import { expect, test } from 'bun:test'
import { formatBenchResult, runMessageViewportBenchmark } from './messageViewportBenchmark'

const SLACK = 5

test('message viewport layout and windowing stay within 5x of the 100k budget', () => {
  const results = runMessageViewportBenchmark()
  const failures = results.filter((result) => result.value > result.budget * SLACK)
  expect(failures.map((result) => formatBenchResult(result, SLACK))).toEqual([])
}, 30_000)

// Migrated from web/src/textFrameBatch.test.ts (TG-011); the scheduler is now a required
// injected interface instead of defaulted requestAnimationFrame.
import { describe, expect, test } from 'bun:test'
import { createTextFrameBatch } from './textFrameBatch'

describe('streaming text frame batching', () => {
  test('coalesces token deltas and flushes the pending frame on completion', () => {
    const output: string[] = []
    let callback: (() => void) | null = null
    const batch = createTextFrameBatch((text) => output.push(text), {
      schedule: (next) => {
        callback = next
        return 7
      },
      cancel: () => {
        callback = null
      },
    })
    batch.push('你')
    batch.push('好')
    expect(output).toEqual([])
    batch.flush()
    expect(callback).toBeNull()
    expect(output).toEqual(['你好'])
  })

  test('flushes once per scheduled frame and then accepts more input', () => {
    const output: string[] = []
    const scheduled: (() => void)[] = []
    const batch = createTextFrameBatch((text) => output.push(text), {
      schedule: (next) => scheduled.push(next) - 1,
      cancel: () => {},
    })
    batch.push('a')
    batch.push('b')
    scheduled[0]?.()
    batch.push('c')
    scheduled[1]?.()
    expect(output).toEqual(['ab', 'c'])
  })
})

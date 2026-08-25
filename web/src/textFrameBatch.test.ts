import { describe, expect, test } from 'bun:test'
import { createTextFrameBatch } from './textFrameBatch'

describe('streaming text frame batching', () => {
  test('coalesces token deltas and flushes the pending frame on completion', () => {
    const output: string[] = []
    let callback: FrameRequestCallback | null = null
    const batch = createTextFrameBatch(
      (text) => output.push(text),
      (next) => {
        callback = next
        return 7
      },
      () => {
        callback = null
      },
    )
    batch.push('你')
    batch.push('好')
    expect(output).toEqual([])
    batch.flush()
    expect(callback).toBeNull()
    expect(output).toEqual(['你好'])
  })
})

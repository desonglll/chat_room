import { describe, expect, test } from 'bun:test'
import { createBatchCache } from '../batchCache'

function harness(ttlMs = Number.POSITIVE_INFINITY) {
  const batches: string[][] = []
  let fail = false
  let now = 0
  let queued: (() => void) | null = null
  const cache = createBatchCache<string>({
    load: async (keys) => {
      batches.push(keys)
      if (fail) throw new Error('offline')
      return new Map(keys.filter((key) => key !== 'missing').map((key) => [key, key.toUpperCase()]))
    },
    maxBatch: 2,
    ttlMs,
    now: () => now,
    schedule: (flush) => {
      queued = flush
    },
  })
  const flush = async () => {
    queued?.()
    queued = null
    await new Promise((resolve) => setTimeout(resolve, 0))
  }
  return { cache, batches, flush, setFail: (value: boolean) => (fail = value), advance: (ms: number) => (now += ms) }
}

describe('createBatchCache', () => {
  test('one render worth of requests becomes batches of maxBatch; missing keys cache as null', async () => {
    const { cache, batches, flush } = harness()
    let notified = 0
    cache.subscribe(() => notified++)
    for (const key of ['a', 'b', 'a', 'missing']) cache.request(key)
    await flush()
    expect(batches).toEqual([['a', 'b'], ['missing']])
    expect(cache.get('a')).toBe('A')
    expect(cache.get('missing')).toBeNull()
    expect(notified).toBe(2)
    cache.request('a')
    await flush()
    expect(batches).toHaveLength(2)
  })

  test('a failed batch stays unknown and is retried; answers expire after the ttl', async () => {
    const { cache, batches, flush, setFail, advance } = harness(1000)
    setFail(true)
    cache.request('a')
    await flush()
    expect(cache.get('a')).toBeUndefined()
    setFail(false)
    cache.request('a')
    await flush()
    expect(cache.get('a')).toBe('A')
    advance(1000)
    cache.request('a')
    await flush()
    expect(batches).toHaveLength(3)
  })
})

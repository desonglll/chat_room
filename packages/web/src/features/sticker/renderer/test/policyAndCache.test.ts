import { describe, expect, test } from 'bun:test'
import { admitPlayback, type PlaybackCandidate } from '../playbackPolicy'
import { createRefCountedCache } from '../refCountedCache'

const view = (id: number, group: string, overrides: Partial<PlaybackCandidate> = {}): PlaybackCandidate => ({
  id,
  group,
  visible: true,
  wantsMotion: true,
  playing: false,
  visibleSince: id,
  ...overrides,
})

const env = { hidden: false, reducedMotion: false, maxGroups: 2 }

describe('admitPlayback', () => {
  test('only visible views that want motion play', () => {
    const admitted = admitPlayback(
      [view(1, 'a'), view(2, 'b', { visible: false }), view(3, 'c', { wantsMotion: false })],
      env,
    )
    expect([...admitted]).toEqual([1])
  })

  test('a hidden page or reduced motion admits nothing', () => {
    expect(admitPlayback([view(1, 'a')], { ...env, hidden: true }).size).toBe(0)
    expect(admitPlayback([view(1, 'a')], { ...env, reducedMotion: true }).size).toBe(0)
  })

  test('the cap counts render groups; extra copies of an admitted sticker are free', () => {
    const admitted = admitPlayback([view(1, 'a'), view(2, 'a'), view(3, 'b'), view(4, 'c'), view(5, 'a')], env)
    expect([...admitted].sort()).toEqual([1, 2, 3, 5])
  })

  test('playing groups keep their slot over longer-visible newcomers', () => {
    const admitted = admitPlayback(
      [
        view(1, 'a', { visibleSince: 1 }),
        view(2, 'b', { visibleSince: 2 }),
        view(3, 'c', { visibleSince: 9, playing: true }),
      ],
      env,
    )
    expect([...admitted].sort()).toEqual([1, 3])
  })
})

describe('refCountedCache', () => {
  test('one create per key while referenced; idle entries evicted LRU beyond the ceiling', async () => {
    const evicted: string[] = []
    let creates = 0
    const cache = createRefCountedCache<string>({ maxIdleCost: 1, cost: () => 1, onEvict: (v) => evicted.push(v) })
    const make = (value: string) => async () => {
      creates += 1
      return value
    }
    const [a1, a2] = await Promise.all([cache.acquire('a', make('A')), cache.acquire('a', make('A'))])
    expect([a1, a2, creates]).toEqual(['A', 'A', 1])
    await cache.acquire('b', make('B'))
    cache.release('a')
    expect(evicted).toEqual([])
    cache.release('a')
    cache.release('b')
    // Two idle entries, ceiling 1: the least recently released ("a") goes.
    expect(evicted).toEqual(['A'])
    expect(cache.peek('b')).toBe('B')
    expect(await cache.acquire('b', make('X'))).toBe('B')
  })

  test('a failed create is not cached', async () => {
    const cache = createRefCountedCache<string>({ maxIdleCost: 10, cost: () => 1 })
    await expect(cache.acquire('k', () => Promise.reject(new Error('boom')))).rejects.toThrow('boom')
    expect(cache.size).toBe(0)
    expect(await cache.acquire('k', async () => 'ok')).toBe('ok')
  })
})

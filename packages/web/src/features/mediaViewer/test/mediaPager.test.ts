import { describe, expect, test } from 'bun:test'
import { createMediaPager } from '../mediaPager'
import type { MediaPage } from '../mediaPager'
import { fakeServer, media } from './fixtures'

const range = (from: number, to: number) => Array.from({ length: to - from + 1 }, (_, i) => from + i)
const ids = (pager: ReturnType<typeof createMediaPager>) => pager.getState().items.map((item) => item.attachmentId)
const expected = (numbers: number[], fileEvery = 0) =>
  numbers.filter((n) => !(fileEvery && n % fileEvery === 0)).map((n) => `a${n}`)

async function drainOlder(pager: ReturnType<typeof createMediaPager>, limit = 50) {
  for (let i = 0; i < limit && pager.getState().hasOlder; i += 1) await pager.loadOlder()
}

describe('seeded from the newest loaded window', () => {
  test('opens on the seed, syncs the head in one page, then walks older history without gaps', async () => {
    const server = fakeServer(60, 10, 7)
    const seed = range(55, 60)
      .filter((n) => n % 7 !== 0)
      .map((n) => media(n))
    const pager = createMediaPager({ seed, targetId: 'a57', fetchPage: server.fetchPage })
    expect(ids(pager)).toEqual(expected(range(55, 60), 7))

    await pager.start()
    expect(server.calls).toEqual([null])
    // The head page (51..60) overlapped the seed; older is still unknown.
    expect(ids(pager)).toEqual(expected(range(51, 60), 7))
    expect(pager.getState().hasOlder).toBe(true)

    await drainOlder(pager)
    expect(ids(pager)).toEqual(expected(range(1, 60), 7))
    expect(pager.getState().hasOlder).toBe(false)
    // Every row page was requested once: no re-fetch loop, no skipped cursor.
    expect(server.calls).toEqual([null, 'm51', 'm41', 'm31', 'm21', 'm11'])
  })

  test('keeps the seed caption when a /files copy (no caption) of the same item arrives', async () => {
    const server = fakeServer(5, 10)
    const pager = createMediaPager({ seed: [media(5)], targetId: 'a5', fetchPage: server.fetchPage })
    await pager.start()
    const five = pager.getState().items.find((item) => item.attachmentId === 'a5')
    expect(five?.caption).toBe('caption 5')
    expect(pager.getState().items.find((item) => item.attachmentId === 'a4')?.caption).toBeNull()
    expect(pager.getState().hasOlder).toBe(false)
  })

  test('media newer than the loaded window is synced before the seed (no silent skip)', async () => {
    const server = fakeServer(50, 10)
    // Loaded window 21..30, but the chat already has media up to 50.
    const seed = range(21, 30).map((n) => media(n))
    const pager = createMediaPager({ seed, targetId: 'a25', fetchPage: server.fetchPage })
    await pager.start()
    expect(server.calls).toEqual([null, 'm41', 'm31'])
    expect(ids(pager)).toEqual(expected(range(21, 50)))
    await drainOlder(pager)
    expect(ids(pager)).toEqual(expected(range(1, 50)))
  })

  test('a window too far from the head keeps the seed alone rather than showing a gap', async () => {
    const server = fakeServer(500, 10)
    const seed = range(21, 30).map((n) => media(n))
    const pager = createMediaPager({ seed, targetId: 'a25', fetchPage: server.fetchPage, maxSyncPages: 3 })
    await pager.start()
    expect(server.calls.length).toBe(3)
    expect(ids(pager)).toEqual(expected(range(21, 30)))
    await pager.loadOlder()
    expect(server.calls.at(-1)).toBe('m21')
    expect(ids(pager)).toEqual(expected(range(11, 30)))
  })
})

describe('without a usable seed', () => {
  test('empty seed: pages from the head until the target is found', async () => {
    const server = fakeServer(40, 10)
    const pager = createMediaPager({ seed: [], targetId: 'a15', fetchPage: server.fetchPage })
    await pager.start()
    expect(server.calls).toEqual([null, 'm31', 'm21'])
    expect(ids(pager)).toEqual(expected(range(11, 40)))
    await pager.loadOlder()
    expect(ids(pager)).toEqual(expected(range(1, 40)))
    expect(pager.getState().hasOlder).toBe(false)
  })

  test('a seed that lacks the target is replaced by the fetched range that has it', async () => {
    const server = fakeServer(100, 10)
    const pager = createMediaPager({ seed: [media(3)], targetId: 'a85', fetchPage: server.fetchPage })
    await pager.start()
    // Pages reach m3 (the seed) only after 10 pages; the cap of 8 stops first.
    expect(ids(pager)).toEqual(expected(range(21, 100)))
  })
})

describe('failures and edits', () => {
  test('a failed sync keeps the seed usable and a later loadOlder retries', async () => {
    let fail = true
    const server = fakeServer(20, 10)
    const fetchPage = async (before: string | null): Promise<MediaPage> => {
      if (fail) throw new Error('offline')
      return server.fetchPage(before)
    }
    const pager = createMediaPager({ seed: [media(19), media(20)], targetId: 'a20', fetchPage })
    await pager.start()
    expect(pager.getState()).toMatchObject({ failed: true, syncing: false })
    expect(ids(pager)).toEqual(['a19', 'a20'])
    fail = false
    await pager.loadOlder()
    expect(pager.getState().failed).toBe(false)
    expect(ids(pager)).toEqual(expected(range(9, 20)))
  })

  test('a removed item never returns from a later page', async () => {
    const server = fakeServer(20, 10)
    const pager = createMediaPager({ seed: [media(20)], targetId: 'a20', fetchPage: server.fetchPage })
    pager.remove('a15')
    await pager.start()
    await drainOlder(pager)
    expect(ids(pager)).not.toContain('a15')
    expect(ids(pager).length).toBe(19)
  })

  test('start is idempotent (StrictMode double effect) and subscribers see every change', async () => {
    const server = fakeServer(10, 10)
    const pager = createMediaPager({ seed: [media(10)], targetId: 'a10', fetchPage: server.fetchPage })
    const seen: boolean[] = []
    const unsubscribe = pager.subscribe((state) => seen.push(state.syncing))
    await Promise.all([pager.start(), pager.start()])
    unsubscribe()
    expect(server.calls).toEqual([null])
    expect(seen).toEqual([true, false])
  })

  test('loadOlder is a no-op while syncing, loading, or exhausted', async () => {
    const server = fakeServer(5, 10)
    const pager = createMediaPager({ seed: [media(5)], targetId: 'a5', fetchPage: server.fetchPage })
    const syncing = pager.start()
    await pager.loadOlder()
    await syncing
    await pager.loadOlder()
    expect(server.calls).toEqual([null])
  })
})

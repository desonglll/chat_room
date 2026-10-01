/** Every tab owns its cursor: paging one never moves another (TG-106 acceptance). */
import { expect, test } from 'bun:test'
import { createSharedPager } from '../sharedPager'
import type { SharedFile, SharedLink, SharedSources } from '../sharedSources'
import { createChatInfoPagers } from '../useChatInfo'
import { fakeClient, fileItem, member } from './fixtures'
import { toSharedFile } from '../sharedSources'

/** A source that pages through `total` items two at a time and logs every cursor it gets. */
function countingSource<T>(make: (n: number) => T, total: number, log: Array<string | null>) {
  return async (cursor: string | null) => {
    log.push(cursor)
    const start = cursor === null ? 0 : Number(cursor)
    const items = Array.from({ length: Math.min(2, total - start) }, (_, i) => make(start + i))
    return { items, next: start + 2 < total ? String(start + 2) : null }
  }
}

const file = (n: number): SharedFile => toSharedFile(fileItem(`f${n}`, 'image/png'))
const link = (n: number): SharedLink => ({
  key: `l${n}`,
  messageId: `m${n}`,
  createdAt: '',
  sender: 'a',
  url: `https://x/${n}`,
  host: 'x',
})

test('paging one tab leaves every other tab cursor, items and request log untouched', async () => {
  const logs = { media: [], files: [], links: [], music: [], voice: [], gif: [] } as Record<
    keyof SharedSources,
    Array<string | null>
  >
  const shared: SharedSources = {
    media: countingSource(file, 6, logs.media),
    files: countingSource(file, 6, logs.files),
    links: countingSource(link, 6, logs.links),
    music: countingSource(file, 6, logs.music),
    voice: countingSource(file, 6, logs.voice),
    gif: countingSource(file, 6, logs.gif),
  }
  const { client } = fakeClient(() => [])
  const pagers = createChatInfoPagers('c1', client, () => 't', { shared, loadMembers: async () => [] })

  await pagers.media.getState().loadMore()
  await pagers.media.getState().loadMore()
  await pagers.links.getState().loadMore()

  expect(logs.media).toEqual([null, '2'])
  expect(logs.links).toEqual([null])
  expect(logs.files).toEqual([])
  expect(pagers.media.getState().cursor).toBe('4')
  expect(pagers.media.getState().items).toHaveLength(4)
  expect(pagers.links.getState().cursor).toBe('2')
  expect(pagers.files.getState()).toMatchObject({ cursor: null, started: false, items: [] })

  // Files starts from ITS first page even though media is two pages deep.
  await pagers.files.getState().loadMore()
  expect(logs.files).toEqual([null])
  await pagers.media.getState().loadMore()
  expect(pagers.media.getState().done).toBe(true)
  expect(pagers.files.getState().done).toBe(false)
})

test('a pager ignores loadMore while loading and after the last page, and dedupes by key', async () => {
  const calls: Array<string | null> = []
  const pager = createSharedPager(
    async (cursor) => {
      calls.push(cursor)
      return cursor === null ? { items: ['a', 'b'], next: 'x' } : { items: ['b', 'c'], next: null }
    },
    (item: string) => item,
  )
  await Promise.all([pager.getState().loadMore(), pager.getState().loadMore()])
  expect(calls).toEqual([null])
  await pager.getState().loadMore()
  await pager.getState().loadMore()
  expect(calls).toEqual([null, 'x'])
  expect(pager.getState().items).toEqual(['a', 'b', 'c'])
  expect(pager.getState().done).toBe(true)
})

test('a failed page keeps the cursor so a retry asks for the same page', async () => {
  let fail = true
  const calls: Array<string | null> = []
  const pager = createSharedPager(
    async (cursor) => {
      calls.push(cursor)
      if (cursor === 'p2' && fail) throw new Error('boom')
      return { items: [cursor ?? 'first'], next: cursor === null ? 'p2' : null }
    },
    (item: string) => item,
  )
  await pager.getState().loadMore()
  await pager.getState().loadMore()
  expect(pager.getState().error).toBe('boom')
  fail = false
  await pager.getState().loadMore()
  expect(calls).toEqual([null, 'p2', 'p2'])
  expect(pager.getState()).toMatchObject({ error: '', done: true, items: ['first', 'p2'] })
})

test('members page client-side over the one-shot listing: owner, admins, online, then name', async () => {
  const { client } = fakeClient(() => [])
  let loads = 0
  const everyone = [member('u9', 'member', 'zed'), member('u1', 'owner', 'amy'), member('u5', 'admin', 'kim')]
  for (let i = 0; i < 60; i += 1) everyone.push(member(`x${i}`, 'member', `m${String(i).padStart(2, '0')}`))
  const pagers = createChatInfoPagers('c1', client, () => 't', {
    shared: {} as SharedSources,
    loadMembers: async () => {
      loads += 1
      return everyone
    },
  })
  await pagers.members.getState().loadMore()
  const first = pagers.members.getState()
  expect(first.items.slice(0, 2).map((m) => m.role)).toEqual(['owner', 'admin'])
  expect(first.items).toHaveLength(50)
  expect(first.cursor).toBe('50')
  await pagers.members.getState().loadMore()
  expect(pagers.members.getState().items).toHaveLength(63)
  expect(pagers.members.getState().done).toBe(true)
  expect(loads).toBe(1)
})

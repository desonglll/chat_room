/** TG-803: each tab reads its own server-classified endpoint; links come from the link index. */
import { expect, test } from 'bun:test'
import { extractLinks } from '../linkExtract'
import { createSharedSources, FILES_PAGE_SIZE, LINKS_PAGE_SIZE, linkHost } from '../sharedSources'
import { fakeClient, fileItem } from './fixtures'

test('media / files / voice / GIF each ask the server for their own kind', async () => {
  const { client, calls } = fakeClient((call) => ({
    items: [fileItem(String(call.query.kind), 'application/octet-stream')],
    next_before: null,
  }))
  const sources = createSharedSources('c1', { client, token: () => 't' })
  for (const tab of ['media', 'files', 'voice', 'gif'] as const) {
    const page = await sources[tab](null)
    expect(page.items).toHaveLength(1)
    expect(page.next).toBeNull()
  }
  expect(calls.map((call) => [call.path, call.query.kind, call.query.limit])).toEqual([
    ['/api/chats/c1/files', 'media', String(FILES_PAGE_SIZE)],
    ['/api/chats/c1/files', 'document', String(FILES_PAGE_SIZE)],
    ['/api/chats/c1/files', 'voice', String(FILES_PAGE_SIZE)],
    ['/api/chats/c1/files', 'gif', String(FILES_PAGE_SIZE)],
  ])
})

test('a file source passes its own cursor as `before` and returns the server cursor', async () => {
  const { client, calls } = fakeClient(() => ({ items: [fileItem('a', 'image/png')], next_before: 'm-a' }))
  const sources = createSharedSources('c1', { client, token: () => 't' })
  expect((await sources.gif('m-older')).next).toBe('m-a')
  expect(calls[0]?.query.before).toBe('m-older')
})

test('links: one row per indexed link, keyed by message + position, server cursor', async () => {
  const row = (id: string, position: number, url: string) => ({
    message_id: id,
    position,
    url,
    sender_id: 'u1',
    sender: 'alice',
    created_at: '2026-09-01T00:00:00Z',
  })
  const { client, calls } = fakeClient(() => ({
    items: [row('m2', 0, 'https://www.b.com/'), row('m1', 0, 'https://a.io/x'), row('m1', 1, 'http://c.dev/')],
    next: 'm1:1',
  }))
  const sources = createSharedSources('c1', { client, token: () => 't' })
  const page = await sources.links('m3:0')
  expect(calls[0]).toMatchObject({
    path: '/api/chats/c1/links',
    query: { before: 'm3:0', limit: String(LINKS_PAGE_SIZE) },
  })
  expect(page.items.map((link) => [link.key, link.host])).toEqual([
    ['m2:0', 'b.com'],
    ['m1:0', 'a.io'],
    ['m1:1', 'c.dev'],
  ])
  expect(page.next).toBe('m1:1')

  const { client: last } = fakeClient(() => ({ items: [] }))
  expect((await createSharedSources('c1', { client: last, token: () => 't' }).links(null)).next).toBeNull()
})

test('linkHost and extractLinks: http(s) only, www. dropped, duplicates collapsed', () => {
  expect(linkHost('https://www.example.com/a')).toBe('example.com')
  expect(extractLinks('see https://x.io/a, https://x.io/a and javascript:alert(1) or ftp://y.z')).toEqual([
    { url: 'https://x.io/a', host: 'x.io' },
  ])
  expect(extractLinks('中文里 https://例子.测试/路径。')).toHaveLength(1)
})

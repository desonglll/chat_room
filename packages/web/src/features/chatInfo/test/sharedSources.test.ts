/** Each tab reads its own endpoint + filter; links come from the chat's message search. */
import { expect, test } from 'bun:test'
import type { StoredMessage } from '@tg/core'
import { extractLinks } from '../linkExtract'
import { createSharedSources, LINK_NEEDLE, LINKS_PAGE_SIZE } from '../sharedSources'
import { fakeClient, fileItem } from './fixtures'

const mixed = [
  fileItem('png', 'image/png'),
  fileItem('gif', 'image/gif'),
  fileItem('mp4', 'video/mp4'),
  fileItem('svg', 'image/svg+xml'),
  fileItem('ogg', 'audio/ogg'),
  fileItem('pdf', 'application/pdf'),
]

function filesClient() {
  return fakeClient((call) => {
    const kind = call.query.kind
    const items = mixed.filter((item) => {
      const mime = item.attachment.mime_type
      if (kind === 'image') return mime.startsWith('image/')
      if (kind === 'video') return mime.startsWith('video/')
      if (kind === 'file') return !mime.startsWith('image/') && !mime.startsWith('video/')
      return true
    })
    return { items, next_before: null }
  })
}

test('media / files / voice / GIF split one mixed listing without overlap', async () => {
  const { client, calls } = filesClient()
  const sources = createSharedSources('c1', { client, token: () => 't' })
  const ids = async (tab: 'media' | 'files' | 'voice' | 'gif') =>
    (await sources[tab](null)).items.map((file) => file.attachment.id)
  expect(await ids('media')).toEqual(['png', 'mp4'])
  expect(await ids('files')).toEqual(['svg', 'pdf'])
  expect(await ids('voice')).toEqual(['ogg'])
  expect(await ids('gif')).toEqual(['gif'])
  expect(calls.map((call) => [call.path, call.query.kind])).toEqual([
    ['/api/chats/c1/files', 'all'],
    ['/api/chats/c1/files', 'all'],
    ['/api/chats/c1/files', 'file'],
    ['/api/chats/c1/files', 'image'],
  ])
})

test('a file source passes its own cursor as `before`', async () => {
  const { client, calls } = fakeClient(() => ({ items: [fileItem('a', 'image/png')], next_before: 'm-a' }))
  const sources = createSharedSources('c1', { client, token: () => 't' })
  await sources.gif('m-older')
  expect(calls[0]?.query.before).toBe('m-older')
})

function stored(id: string, content: string): StoredMessage {
  return {
    id,
    room_id: 'c1',
    client_message_id: null,
    sender_id: 'u1',
    sender: 'alice',
    sender_avatar: '',
    content,
    attachment: null,
    reply_to: null,
    recalled_at: null,
    edited_at: null,
    created_at: '2026-09-01T00:00:00Z',
    favorite_id: null,
    forwarded_from: null,
    reactions: [],
  }
}

test('links: searched with the needle, one row per URL, cursor = last message of a full page', async () => {
  const full = Array.from({ length: LINKS_PAGE_SIZE }, (_, i) =>
    stored(
      `m${i}`,
      i === 0 ? 'a https://a.io/x and http://www.b.com.' : i % 2 ? `https://c.dev/${i}` : 'mentions http only',
    ),
  )
  const { client, calls } = fakeClient(() => full)
  const sources = createSharedSources('c1', { client, token: () => 't' })
  const page = await sources.links(null)
  expect(calls[0]).toMatchObject({ path: '/api/chats/c1/messages/search', query: { q: LINK_NEEDLE } })
  expect(page.items.slice(0, 3).map((link) => [link.url, link.host])).toEqual([
    ['https://a.io/x', 'a.io'],
    ['http://www.b.com', 'b.com'],
    ['https://c.dev/1', 'c.dev'],
  ])
  expect(page.items).toHaveLength(2 + LINKS_PAGE_SIZE / 2)
  expect(calls).toHaveLength(1) // enough links in one page: no extra round trip
  expect(page.next).toBe(`m${LINKS_PAGE_SIZE - 1}`)

  const { client: short } = fakeClient(() => [stored('z', 'https://z.dev')])
  expect((await createSharedSources('c1', { client: short, token: () => 't' }).links('m9')).next).toBeNull()
})

test('extractLinks: http(s) only, trailing punctuation dropped, duplicates collapsed', () => {
  expect(extractLinks('see https://x.io/a, https://x.io/a and javascript:alert(1) or ftp://y.z')).toEqual([
    { url: 'https://x.io/a', host: 'x.io' },
  ])
  expect(extractLinks('中文里 https://例子.测试/路径。')).toHaveLength(1)
})

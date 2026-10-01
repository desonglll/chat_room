import { describe, expect, test } from 'bun:test'
import { classify, entriesToEvict, pushContent } from '../cachePolicy'

const origin = 'https://chat.example'
const at = (path: string) => new URL(path, origin)

describe('TG-601 service worker policy', () => {
  test('requests are routed by kind; writes, sockets and other origins are untouched', () => {
    expect(classify('GET', at('/chat/1'), origin, true)).toBe('shell')
    expect(classify('GET', at('/assets/index-abc.js'), origin, false)).toBe('asset')
    expect(classify('GET', at('/api/conversations'), origin, false)).toBe('api-read')
    expect(classify('GET', at('/api/chats/c1/messages'), origin, false)).toBe('api-read')
    expect(classify('GET', at('/api/attachments/a1/download'), origin, false)).toBe('media')
    expect(classify('GET', at('/api/users/u1/avatar'), origin, false)).toBe('media')
    expect(classify('POST', at('/api/chats/c1/messages'), origin, false)).toBe('bypass')
    expect(classify('GET', at('/api/messages/search'), origin, false)).toBe('bypass')
    expect(classify('GET', new URL('https://tile.openstreetmap.org/1/1/1.png'), origin, false)).toBe('bypass')
  })

  test('media eviction honours retention first, then the size limit from the oldest', () => {
    const day = 86_400_000
    const now = 100 * day
    const mb = 1024 * 1024
    const entries = [
      { url: 'old', bytes: mb, cachedAt: now - 10 * day },
      { url: 'a', bytes: 400 * mb, cachedAt: now - 3 * day },
      { url: 'b', bytes: 400 * mb, cachedAt: now - 2 * day },
      { url: 'c', bytes: 400 * mb, cachedAt: now - day },
    ]
    expect(entriesToEvict(entries, { limitMb: 1024, retentionDays: 7 }, now).sort()).toEqual(['a', 'old'])
    expect(entriesToEvict(entries, { limitMb: 10_000, retentionDays: 0 }, now)).toEqual([])
  })

  test('a push opens the message it is about; junk falls back safely', () => {
    const content = pushContent({ title: 'Echo Gate', body: 'hi', url: '/chat/c1?message=m1', tag: 'n:1' })
    expect(content.options.data.url).toBe('/chat/c1?message=m1')
    expect(content.options.body).toBe('hi')
    expect(pushContent({ url: 'https://evil.example/' }).options.data.url).toBe('/')
    expect(pushContent(null).title).toBe('Echo Gate')
  })
})

import { describe, expect, test } from 'bun:test'
import { firstLink } from './linkPreviews'

describe('TG-408 firstLink', () => {
  test('matches the server: first http(s) link, trailing punctuation dropped', () => {
    expect(firstLink('see https://example.com/a).')).toBe('https://example.com/a')
    expect(firstLink('a http://x.org, then https://y.org')).toBe('http://x.org')
    expect(firstLink('看这个：https://例子.测试/路径。')).toBe('https://例子.测试/路径')
    expect(firstLink('ftp://example.com')).toBeNull()
    expect(firstLink('no links')).toBeNull()
  })
})

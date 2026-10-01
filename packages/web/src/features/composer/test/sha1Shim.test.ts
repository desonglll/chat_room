import { describe, expect, test } from 'bun:test'
import { createHash } from 'node:crypto'
import { ensureSubtleDigest, sha1, sha1OnlySubtle } from '../sha1Shim'

const hex = (bytes: ArrayBuffer | Uint8Array) =>
  Buffer.from(bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes)).toString('hex')
const utf8 = (text: string) => new TextEncoder().encode(text)

describe('TG-1301 SHA-1 fallback', () => {
  test('matches the FIPS 180 test vectors', () => {
    expect(hex(sha1(utf8('')))).toBe('da39a3ee5e6b4b0d3255bfef95601890afd80709')
    expect(hex(sha1(utf8('abc')))).toBe('a9993e364706816aba3e25717850c26c9cd0d89d')
    expect(hex(sha1(utf8('abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq')))).toBe(
      '84983e441c3bd26ebaae4aa1f95129e5e54670f1',
    )
    expect(hex(sha1(new Uint8Array(1_000_000).fill(0x61)))).toBe('34aa973cd4c4daa4f61eeb2bdbad27316534016f')
  })

  test('agrees with node at every length around the padding boundaries', () => {
    for (let length = 0; length <= 130; length++) {
      const input = Uint8Array.from({ length }, (_, i) => (i * 31 + 7) & 0xff)
      expect(hex(sha1(input))).toBe(createHash('sha1').update(input).digest('hex'))
    }
  })

  test('digest takes what emoji-picker-element passes (an ArrayBuffer) and views of a larger buffer', async () => {
    const backing = utf8('xxabcxx')
    expect(hex(await sha1OnlySubtle.digest('SHA-1', backing.slice(2, 5).buffer))).toBe(
      'a9993e364706816aba3e25717850c26c9cd0d89d',
    )
    expect(hex(await sha1OnlySubtle.digest({ name: 'sha-1' }, backing.subarray(2, 5)))).toBe(
      'a9993e364706816aba3e25717850c26c9cd0d89d',
    )
    await expect(sha1OnlySubtle.digest('SHA-256', backing)).rejects.toThrow('SHA-256')
  })

  test('installs only when crypto.subtle is missing', () => {
    const real = { digest: () => 'native' }
    const secure = { crypto: { subtle: real } }
    expect(ensureSubtleDigest(secure)).toBe(false)
    expect(secure.crypto.subtle).toBe(real)

    // An insecure context: `crypto` (getRandomValues) exists, `subtle` does not.
    const insecure: { crypto?: { subtle?: unknown } } = { crypto: {} }
    expect(ensureSubtleDigest(insecure)).toBe(true)
    expect(insecure.crypto?.subtle).toBe(sha1OnlySubtle)
    expect(ensureSubtleDigest(insecure)).toBe(false)

    const bare: { crypto?: { subtle?: unknown } } = {}
    expect(ensureSubtleDigest(bare)).toBe(true)
    expect(bare.crypto?.subtle).toBe(sha1OnlySubtle)
  })
})

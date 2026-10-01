/**
 * TG-1301: `emoji-picker-element` checksums its emoji data with `crypto.subtle.digest('SHA-1')`
 * whenever the data response carries no ETag (ours does not). `crypto.subtle` exists only in a
 * secure context, so on a phone that opens the app over `http://<LAN IP>` the picker threw and the
 * emoji panel showed only its error line.
 *
 * `ensureSubtleDigest()` installs a SHA-1-only `digest` when — and only when — `crypto.subtle` is
 * missing. The checksum is a cache key, not a security boundary, so a plain JS SHA-1 is enough.
 * Where the platform has `crypto.subtle` nothing changes.
 */

const rotl = (value: number, bits: number) => (value << bits) | (value >>> (32 - bits))

/** SHA-1 of `input` (FIPS 180-4, section 6.1). */
export function sha1(input: Uint8Array): Uint8Array {
  const bitLength = input.length * 8
  // Message + 0x80 + zero padding + 64-bit length, rounded up to whole 64-byte blocks.
  const padded = new Uint8Array((((input.length + 8) >> 6) + 1) << 6)
  padded.set(input)
  padded[input.length] = 0x80
  const view = new DataView(padded.buffer)
  view.setUint32(padded.length - 8, Math.floor(bitLength / 0x1_0000_0000))
  view.setUint32(padded.length - 4, bitLength >>> 0)

  const h = [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476, 0xc3d2e1f0]
  const w = new Uint32Array(80)
  for (let block = 0; block < padded.length; block += 64) {
    for (let i = 0; i < 16; i++) w[i] = view.getUint32(block + i * 4)
    for (let i = 16; i < 80; i++) w[i] = rotl(w[i - 3]! ^ w[i - 8]! ^ w[i - 14]! ^ w[i - 16]!, 1)
    let [a, b, c, d, e] = h as [number, number, number, number, number]
    for (let i = 0; i < 80; i++) {
      const [f, k] =
        i < 20
          ? [(b & c) | (~b & d), 0x5a827999]
          : i < 40
            ? [b ^ c ^ d, 0x6ed9eba1]
            : i < 60
              ? [(b & c) | (b & d) | (c & d), 0x8f1bbcdc]
              : [b ^ c ^ d, 0xca62c1d6]
      const next = (rotl(a, 5) + f + e + k + w[i]!) >>> 0
      e = d
      d = c
      c = rotl(b, 30) >>> 0
      b = a
      a = next
    }
    h[0] = (h[0]! + a) >>> 0
    h[1] = (h[1]! + b) >>> 0
    h[2] = (h[2]! + c) >>> 0
    h[3] = (h[3]! + d) >>> 0
    h[4] = (h[4]! + e) >>> 0
  }

  const out = new Uint8Array(20)
  const outView = new DataView(out.buffer)
  h.forEach((word, i) => outView.setUint32(i * 4, word))
  return out
}

type DigestInput = ArrayBuffer | ArrayBufferView

function bytesOf(data: DigestInput): Uint8Array {
  if (data instanceof ArrayBuffer) return new Uint8Array(data)
  return new Uint8Array(data.buffer, data.byteOffset, data.byteLength)
}

/** The fallback `crypto.subtle`: SHA-1 `digest` only; anything else rejects like an unsupported algorithm. */
export const sha1OnlySubtle = {
  digest(algorithm: string | { name: string }, data: DigestInput): Promise<ArrayBuffer> {
    const name = (typeof algorithm === 'string' ? algorithm : algorithm.name).toUpperCase()
    if (name !== 'SHA-1') return Promise.reject(new DOMException(`${name} is not available`, 'NotSupportedError'))
    return Promise.resolve(sha1(bytesOf(data)).buffer as ArrayBuffer)
  },
}

/** Installs `sha1OnlySubtle` as `crypto.subtle` if the platform has none. Returns whether it did. */
export function ensureSubtleDigest(scope: { crypto?: { subtle?: unknown } } = globalThis): boolean {
  if (scope.crypto?.subtle) return false
  if (!scope.crypto) {
    Object.defineProperty(scope, 'crypto', { value: {}, configurable: true, writable: true })
  }
  Object.defineProperty(scope.crypto, 'subtle', { value: sha1OnlySubtle, configurable: true })
  return true
}

/**
 * Where a sticker's bytes come from. TG-302 defines the sticker message wire shape; this
 * renderer only needs a URL (fetched same-origin, so attachment authorization applies) or
 * bytes already in memory (upload preview, cache).
 */

export type StickerSource = { url: string } | { bytes: Uint8Array; key?: string }

const byteKeys = new WeakMap<Uint8Array, string>()
let anonymous = 0

/** Cache identity. Bytes without a caller-provided key are keyed by object identity. */
export function sourceKey(source: StickerSource): string {
  if ('url' in source) return `url:${source.url}`
  if (source.key !== undefined) return `key:${source.key}`
  let key = byteKeys.get(source.bytes)
  if (key === undefined) {
    key = `bytes:${++anonymous}`
    byteKeys.set(source.bytes, key)
  }
  return key
}

export async function fetchStickerBytes(url: string): Promise<Uint8Array> {
  const response = await fetch(url, { credentials: 'same-origin' })
  if (!response.ok) throw new Error(`sticker fetch failed: ${response.status}`)
  return new Uint8Array(await response.arrayBuffer())
}

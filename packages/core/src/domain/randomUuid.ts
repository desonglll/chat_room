/**
 * RFC 4122 v4 UUIDs for `client_message_id` and friends. Migrated from
 * `web/src/randomUuid.ts` (TG-011) with one change: the default source is
 * `globalThis.crypto` (a standards-level global, present in browsers, bun and RN-with-
 * polyfill) instead of the banned `window.crypto`. Still injectable for hosts without it.
 */
export interface RandomCryptoSource {
  getRandomValues(bytes: Uint8Array): Uint8Array
  randomUUID?: () => string
}

export function createRandomUuid(source: RandomCryptoSource = globalThis.crypto): string {
  if (typeof source.randomUUID === 'function') return source.randomUUID()

  const bytes = source.getRandomValues(new Uint8Array(16))
  bytes[6] = ((bytes[6] as number) & 0x0f) | 0x40
  bytes[8] = ((bytes[8] as number) & 0x3f) | 0x80
  const hex = Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('')
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`
}

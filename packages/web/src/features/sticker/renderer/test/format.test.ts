/** Format detection (wire fields, then magic bytes) and the WebM capability rule. */
import { describe, expect, test } from 'bun:test'
import { gzipSync } from 'node:zlib'
import { sniffStickerFormat, stickerFromMessage, stickerUrl, wireStickerFormat } from '../stickerFormat'
import { detectWebmStickerSupport, isAppleWebKit } from '../webmSupport'

const ascii = (text: string) => [...text].map((char) => char.charCodeAt(0))
const RIFF_WEBP = new Uint8Array([...ascii('RIFF'), 0x24, 0, 0, 0, ...ascii('WEBPVP8 ')])
const EBML = new Uint8Array([0x1a, 0x45, 0xdf, 0xa3, 0x9f, 0x42, 0x86, 0x81, 0x01])

describe('wireStickerFormat', () => {
  test('TG-302 `format` wins, case-insensitively', () => {
    expect(wireStickerFormat({ format: 'webm', mime_type: 'image/webp' })).toBe('webm')
    expect(wireStickerFormat({ format: 'TGS' })).toBe('tgs')
  })

  test('falls back to mime_type, then to null (sniff)', () => {
    expect(wireStickerFormat({ mime_type: 'application/x-tgsticker' })).toBe('tgs')
    expect(wireStickerFormat({ format: 'avif', mime_type: 'video/webm; codecs="vp9"' })).toBe('webm')
    expect(wireStickerFormat({ format: 'avif', mime_type: 'application/octet-stream' })).toBeNull()
    expect(wireStickerFormat({})).toBeNull()
  })
})

describe('sniffStickerFormat', () => {
  test('gzip → tgs, RIFF/WEBP → webp, EBML → webm', () => {
    expect(sniffStickerFormat(new Uint8Array(gzipSync('{"v":"5"}')))).toBe('tgs')
    expect(sniffStickerFormat(RIFF_WEBP)).toBe('webp')
    expect(sniffStickerFormat(EBML)).toBe('webm')
  })

  test('raw Lottie JSON (with BOM / whitespace) → tgs path', () => {
    expect(sniffStickerFormat(new Uint8Array(ascii(' \n{"v":"5.7"}')))).toBe('tgs')
    expect(sniffStickerFormat(new Uint8Array([0xef, 0xbb, 0xbf, ...ascii('{}')]))).toBe('tgs')
  })

  test('RIFF that is not WEBP, PNG, truncated and empty input → null', () => {
    expect(sniffStickerFormat(new Uint8Array([...ascii('RIFF'), 0, 0, 0, 0, ...ascii('WAVE')]))).toBeNull()
    expect(sniffStickerFormat(new Uint8Array([0x89, 0x50, 0x4e, 0x47]))).toBeNull()
    expect(sniffStickerFormat(new Uint8Array([0x1a, 0x45]))).toBeNull()
    expect(sniffStickerFormat(new Uint8Array())).toBeNull()
  })
})

describe('wire shapes', () => {
  test('a sticker message renders from its per-message attachment URL', () => {
    const descriptor = stickerFromMessage({
      sticker: { format: 'webm', emoji: '😀', width: 512, height: 512 },
      attachment: { download_url: '/api/attachments/a?key=k', mime_type: 'video/webm' },
    })
    expect(descriptor).toMatchObject({ format: 'webm', emoji: '😀', mime_type: 'video/webm' })
    expect(stickerUrl(descriptor!)).toBe('/api/attachments/a?key=k')
  })

  test('not a sticker message (or recalled: no sticker block) → null', () => {
    expect(stickerFromMessage({ attachment: { download_url: '/x' } })).toBeNull()
    expect(stickerFromMessage({ sticker: { format: 'tgs' }, attachment: null })).toBeNull()
  })

  test('a catalogue sticker uses file_url', () => {
    expect(stickerUrl({ file_url: '/api/stickers/1/file?key=k' })).toBe('/api/stickers/1/file?key=k')
  })
})

describe('WebM (VP9 alpha) capability', () => {
  const CHROME = 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36'
  const SAFARI =
    'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.6 Safari/605.1.15'
  const IOS_CHROME =
    'Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/120.0 Mobile/15E148 Safari/604.1'
  const FIREFOX = 'Mozilla/5.0 (X11; Linux x86_64; rv:130.0) Gecko/20100101 Firefox/130.0'
  const vp9 = (type: string) => (type.includes('vp9') ? 'probably' : '')

  test('Chromium and Firefox with VP9 → supported', () => {
    expect(detectWebmStickerSupport({ canPlayType: vp9, userAgent: CHROME })).toBe(true)
    expect(detectWebmStickerSupport({ canPlayType: vp9, userAgent: FIREFOX })).toBe(true)
  })

  test('no VP9 WebM at all → fallback', () => {
    expect(detectWebmStickerSupport({ canPlayType: () => '', userAgent: CHROME })).toBe(false)
  })

  test('Apple WebKit (Safari, any iOS browser) → fallback even when it claims VP9', () => {
    expect(isAppleWebKit(SAFARI)).toBe(true)
    expect(isAppleWebKit(IOS_CHROME)).toBe(true)
    expect(isAppleWebKit(CHROME)).toBe(false)
    expect(isAppleWebKit(CHROME.replace('Chrome/', 'HeadlessChrome/'))).toBe(false)
    expect(detectWebmStickerSupport({ canPlayType: vp9, userAgent: SAFARI })).toBe(false)
  })
})

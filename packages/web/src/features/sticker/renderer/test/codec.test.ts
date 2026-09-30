import { describe, expect, test } from 'bun:test'
import { gzip } from 'pako'
import { TgsError, readLottieHeader, tgsSpecViolations } from '@tg/core/domain'
import { decodeStickerBytes, pakoInflate } from '../tgsCodec'
import { syntheticLottie } from '../fixtures/syntheticLottie'

const codeOf = (run: () => unknown) => {
  try {
    run()
  } catch (error) {
    return error instanceof TgsError ? error.code : 'foreign'
  }
  return 'no-throw'
}

describe('tgsCodec', () => {
  test('decodes a synthetic .tgs and a raw Lottie .json to the same header', () => {
    const text = JSON.stringify(syntheticLottie({ seed: 3 }))
    const fromTgs = decodeStickerBytes(gzip(text))
    const fromJson = decodeStickerBytes(new TextEncoder().encode(text))
    expect(fromTgs.header).toEqual(fromJson.header)
    expect(fromTgs.header).toMatchObject({ width: 512, height: 512, frameRate: 60, frames: 180, layerCount: 24 })
  })

  test('pako inflate stops a bomb at the bound, mid-stream', () => {
    const bomb = gzip(new Uint8Array(16 * 1024 * 1024))
    expect(() => pakoInflate(bomb, 1024 * 1024)).toThrow(TgsError)
    expect(codeOf(() => decodeStickerBytes(bomb))).toBe('json-too-large')
  })

  test('truncated and non-sticker bytes are rejected with codes', () => {
    const whole = gzip(JSON.stringify(syntheticLottie({ seed: 1 })))
    expect(codeOf(() => decodeStickerBytes(whole.slice(0, whole.length / 2)))).toBe('inflate-failed')
    expect(codeOf(() => decodeStickerBytes(new Uint8Array([1, 2, 3])))).toBe('not-gzip')
    expect(codeOf(() => decodeStickerBytes(gzip('{"v":"5.7.4"}')))).toBe('invalid-lottie')
  })
})

describe('syntheticLottie fixtures', () => {
  test('are deterministic, spec-conformant at the default size, and scale with layers', () => {
    expect(JSON.stringify(syntheticLottie({ seed: 9 }))).toBe(JSON.stringify(syntheticLottie({ seed: 9 })))
    const bytes = gzip(JSON.stringify(syntheticLottie({ seed: 9 })))
    expect(tgsSpecViolations(readLottieHeader(syntheticLottie({ seed: 9 })), bytes.length)).toEqual([])
    const heavy = gzip(JSON.stringify(syntheticLottie({ seed: 9, layers: 60 })))
    expect(heavy.length).toBeGreaterThan(bytes.length * 2)
    expect(heavy.length).toBeLessThan(64 * 1024)
  })
})

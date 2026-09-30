import { describe, expect, test } from 'bun:test'
import { gunzipSync, gzipSync } from 'node:zlib'
import {
  TGS_RENDER_LIMITS,
  TgsError,
  decodeTgs,
  isGzip,
  parseLottieText,
  readLottieHeader,
  tgsSpecViolations,
  type TgsInflate,
} from './tgs'

const inflate: TgsInflate = (bytes, max) => gunzipSync(bytes, { maxOutputLength: max }).toString('utf8')

const lottie = (overrides: Record<string, unknown> = {}) => ({
  v: '5.7.4',
  fr: 60,
  ip: 0,
  op: 180,
  w: 512,
  h: 512,
  layers: [{ ty: 4, ind: 1 }],
  ...overrides,
})

const tgs = (json: unknown) => new Uint8Array(gzipSync(JSON.stringify(json)))

const codeOf = (run: () => unknown): string => {
  try {
    run()
  } catch (error) {
    return error instanceof TgsError ? error.code : `foreign:${String(error)}`
  }
  return 'no-throw'
}

describe('decodeTgs', () => {
  test('decodes a conformant sticker and reports its header', () => {
    const decoded = decodeTgs(tgs(lottie()), inflate)
    expect(decoded.header).toEqual({
      version: '5.7.4',
      width: 512,
      height: 512,
      frameRate: 60,
      inPoint: 0,
      outPoint: 180,
      frames: 180,
      durationMs: 3000,
      layerCount: 1,
    })
    expect(decoded.animation.layers).toEqual([{ ty: 4, ind: 1 }])
    expect(decoded.jsonLength).toBe(JSON.stringify(lottie()).length)
  })

  test('rejects empty, non-gzip and oversized files before inflating', () => {
    let calls = 0
    const counting: TgsInflate = (bytes, max) => {
      calls += 1
      return inflate(bytes, max)
    }
    expect(codeOf(() => decodeTgs(new Uint8Array(), counting))).toBe('empty')
    expect(codeOf(() => decodeTgs(new TextEncoder().encode('{"v":"5"}'), counting))).toBe('not-gzip')
    const big = new Uint8Array(TGS_RENDER_LIMITS.maxFileBytes + 1)
    big.set([0x1f, 0x8b, 0x08])
    expect(codeOf(() => decodeTgs(big, counting))).toBe('file-too-large')
    expect(calls).toBe(0)
  })

  test('a gzip bomb is stopped by the inflate bound, not after allocation', () => {
    const bomb = new Uint8Array(gzipSync(Buffer.alloc(8 * 1024 * 1024, 0x20)))
    expect(bomb.length).toBeLessThan(TGS_RENDER_LIMITS.maxFileBytes)
    expect(codeOf(() => decodeTgs(bomb, inflate))).toBe('inflate-failed')
  })

  test('corrupt deflate data and non-JSON payloads get distinct codes', () => {
    const corrupt = tgs(lottie())
    corrupt.fill(0xff, 10, 30)
    expect(codeOf(() => decodeTgs(corrupt, inflate))).toBe('inflate-failed')
    expect(codeOf(() => decodeTgs(new Uint8Array(gzipSync('not json')), inflate))).toBe('invalid-json')
  })

  test('an adapter that throws TgsError itself keeps its code', () => {
    const refusing: TgsInflate = () => {
      throw new TgsError('json-too-large')
    }
    expect(codeOf(() => decodeTgs(tgs(lottie()), refusing))).toBe('json-too-large')
  })
})

describe('readLottieHeader', () => {
  test.each([
    ['not an object', []],
    ['missing version', lottie({ v: undefined })],
    ['zero width', lottie({ w: 0 })],
    ['huge height', lottie({ h: 5000 })],
    ['NaN frame rate', lottie({ fr: Number.NaN })],
    ['frame rate above ceiling', lottie({ fr: 240 })],
    ['out before in', lottie({ ip: 10, op: 10 })],
    ['layers not an array', lottie({ layers: {} })],
    ['layer not an object', lottie({ layers: [1] })],
    ['longer than ten seconds', lottie({ fr: 30, op: 301 })],
  ])('rejects %s', (_label, json) => {
    expect(codeOf(() => readLottieHeader(json))).toBe('invalid-lottie')
  })

  test('accepts a lenient 30 fps, 4 s, non-square document for rendering', () => {
    const header = readLottieHeader(lottie({ fr: 30, op: 120, w: 400, h: 300 }))
    expect(header.durationMs).toBe(4000)
  })

  test('limits are overridable', () => {
    expect(codeOf(() => readLottieHeader(lottie(), { ...TGS_RENDER_LIMITS, maxLayers: 0 }))).toBe('invalid-lottie')
  })
})

describe('parseLottieText', () => {
  test('enforces the JSON ceiling for already-inflated text', () => {
    const text = JSON.stringify(lottie())
    expect(codeOf(() => parseLottieText(text, { ...TGS_RENDER_LIMITS, maxJsonBytes: 10 }))).toBe('json-too-large')
    expect(parseLottieText(text).header.frames).toBe(180)
  })
})

describe('tgsSpecViolations', () => {
  test('a conformant sticker has none; each rule reports independently', () => {
    const ok = readLottieHeader(lottie())
    expect(tgsSpecViolations(ok, 20_000)).toEqual([])
    expect(tgsSpecViolations(readLottieHeader(lottie({ op: 181 })), 20_000)).toEqual([])
    expect(tgsSpecViolations(ok, 70_000)).toEqual(['file-size'])
    expect(tgsSpecViolations(readLottieHeader(lottie({ w: 256 })), 1)).toEqual(['dimensions'])
    expect(tgsSpecViolations(readLottieHeader(lottie({ fr: 30, op: 90 })), 1)).toEqual(['frame-rate'])
    expect(tgsSpecViolations(readLottieHeader(lottie({ op: 240 })), 1)).toEqual(['duration'])
  })
})

test('isGzip checks the magic and the deflate method byte', () => {
  expect(isGzip(tgs(lottie()))).toBe(true)
  expect(isGzip(new Uint8Array([0x1f, 0x8b]))).toBe(false)
  expect(isGzip(new Uint8Array([0x1f, 0x8b, 0x00]))).toBe(false)
})

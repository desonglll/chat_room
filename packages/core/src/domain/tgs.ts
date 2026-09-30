/**
 * TGS (Telegram animated sticker) decoding rules — TG-301.
 *
 * A `.tgs` file is a gzip-compressed Lottie JSON document. This module owns everything about
 * that format that is not rendering: the gzip magic check, size ceilings (a 64 KiB file can
 * inflate to many megabytes, so the inflated size is bounded *while* inflating, not after),
 * and validation of the Lottie header fields a renderer depends on.
 *
 * The inflate primitive is injected (`TgsInflate`). `pako` is outside this package's
 * dependency grant, and a React Native host would rather use a native zlib; `packages/web`
 * adapts pako in `features/sticker/renderer/tgsCodec.ts`, the tests adapt Bun's zlib.
 *
 * Two limit sets on purpose:
 * - `TGS_RENDER_LIMITS` is what a client is willing to *render*. It is lenient (stickers made
 *   by other tools are sometimes 30 fps or 4 s long) but bounded, so a hostile file cannot
 *   pin the CPU or memory.
 * - `tgsSpecViolations()` checks Telegram's published sticker spec (512×512, ≤ 3 s, 60 fps,
 *   ≤ 64 KiB). That is an *upload* rule — the server (TG-302) enforces it; a client may use
 *   it for an early, friendlier error.
 */

export type TgsErrorCode =
  | 'empty'
  | 'file-too-large'
  | 'not-gzip'
  | 'inflate-failed'
  | 'json-too-large'
  | 'invalid-json'
  | 'invalid-lottie'

export class TgsError extends Error {
  readonly code: TgsErrorCode

  constructor(code: TgsErrorCode, detail?: string) {
    super(detail ? `${code}: ${detail}` : code)
    this.name = 'TgsError'
    this.code = code
  }
}

export interface TgsLimits {
  /** Compressed file ceiling. */
  maxFileBytes: number
  /** Inflated JSON ceiling, enforced during inflation (zip-bomb guard). */
  maxJsonBytes: number
  maxDimension: number
  minFrameRate: number
  maxFrameRate: number
  maxDurationMs: number
  maxLayers: number
}

export const TGS_RENDER_LIMITS: Readonly<TgsLimits> = Object.freeze({
  maxFileBytes: 256 * 1024,
  maxJsonBytes: 4 * 1024 * 1024,
  maxDimension: 1024,
  minFrameRate: 1,
  maxFrameRate: 120,
  maxDurationMs: 10_000,
  maxLayers: 1000,
})

/** Telegram's animated-sticker requirements (core.telegram.org/stickers). */
export const TGS_SPEC = Object.freeze({
  maxFileBytes: 64 * 1024,
  dimension: 512,
  frameRate: 60,
  maxDurationMs: 3000,
})

/**
 * Inflates gzip bytes to UTF-8 text. Must throw (anything) once more than `maxOutputBytes`
 * would be produced — the adapter enforces the bound incrementally, never after the fact.
 */
export type TgsInflate = (compressed: Uint8Array, maxOutputBytes: number) => string

export interface LottieHeader {
  version: string
  width: number
  height: number
  frameRate: number
  inPoint: number
  outPoint: number
  /** `outPoint - inPoint`. */
  frames: number
  durationMs: number
  layerCount: number
}

export interface DecodedTgs {
  header: LottieHeader
  /** The parsed Lottie document, handed to the renderer as-is. */
  animation: Record<string, unknown>
  /** Inflated JSON length in UTF-16 code units — the cache's cost estimate. */
  jsonLength: number
}

/** gzip member header: ID1 ID2 CM(8 = deflate). */
export function isGzip(bytes: Uint8Array): boolean {
  return bytes.length >= 3 && bytes[0] === 0x1f && bytes[1] === 0x8b && bytes[2] === 0x08
}

export function decodeTgs(bytes: Uint8Array, inflate: TgsInflate, limits: TgsLimits = TGS_RENDER_LIMITS): DecodedTgs {
  if (bytes.length === 0) throw new TgsError('empty')
  if (bytes.length > limits.maxFileBytes) throw new TgsError('file-too-large', `${bytes.length} bytes`)
  if (!isGzip(bytes)) throw new TgsError('not-gzip')
  let text: string
  try {
    text = inflate(bytes, limits.maxJsonBytes)
  } catch (error) {
    if (error instanceof TgsError) throw error
    throw new TgsError('inflate-failed', error instanceof Error ? error.message : undefined)
  }
  return parseLottieText(text, limits)
}

/** For documents that arrive already inflated (plain `.json` Lottie). */
export function parseLottieText(text: string, limits: TgsLimits = TGS_RENDER_LIMITS): DecodedTgs {
  // UTF-16 length ≤ UTF-8 byte length, so this can only under-count multi-byte text; the
  // inflate adapter has already enforced the byte bound for the gzip path.
  if (text.length > limits.maxJsonBytes) throw new TgsError('json-too-large')
  let parsed: unknown
  try {
    parsed = JSON.parse(text)
  } catch {
    throw new TgsError('invalid-json')
  }
  const header = readLottieHeader(parsed, limits)
  return { header, animation: parsed as Record<string, unknown>, jsonLength: text.length }
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

const finite = (value: unknown): value is number => typeof value === 'number' && Number.isFinite(value)

function invalid(detail: string): never {
  throw new TgsError('invalid-lottie', detail)
}

export function readLottieHeader(json: unknown, limits: TgsLimits = TGS_RENDER_LIMITS): LottieHeader {
  if (!isRecord(json)) invalid('not an object')
  const { v, w, h, fr, ip, op, layers } = json
  if (typeof v !== 'string' || !/^\d+\.\d+/u.test(v)) invalid('version')
  if (!finite(w) || !finite(h) || w <= 0 || h <= 0) invalid('dimensions')
  if (w > limits.maxDimension || h > limits.maxDimension) invalid('dimensions too large')
  if (!finite(fr) || fr < limits.minFrameRate || fr > limits.maxFrameRate) invalid('frame rate')
  if (!finite(ip) || !finite(op) || op <= ip) invalid('in/out point')
  if (!Array.isArray(layers)) invalid('layers')
  if (layers.length > limits.maxLayers) invalid('too many layers')
  if (!layers.every(isRecord)) invalid('layer shape')
  const frames = op - ip
  const durationMs = (frames / fr) * 1000
  if (durationMs > limits.maxDurationMs) invalid('too long')
  return {
    version: v,
    width: w,
    height: h,
    frameRate: fr,
    inPoint: ip,
    outPoint: op,
    frames,
    durationMs,
    layerCount: layers.length,
  }
}

export type TgsSpecViolation = 'file-size' | 'dimensions' | 'frame-rate' | 'duration'

/** Deviations from Telegram's upload spec. An empty list means the sticker is conformant. */
export function tgsSpecViolations(header: LottieHeader, fileBytes: number): TgsSpecViolation[] {
  const violations: TgsSpecViolation[] = []
  if (fileBytes > TGS_SPEC.maxFileBytes) violations.push('file-size')
  if (header.width !== TGS_SPEC.dimension || header.height !== TGS_SPEC.dimension) violations.push('dimensions')
  if (header.frameRate !== TGS_SPEC.frameRate) violations.push('frame-rate')
  // One frame of slack: exporters commonly emit op = 181 for a 3 s / 60 fps sticker.
  if (header.frames > (TGS_SPEC.maxDurationMs / 1000) * header.frameRate + 1) violations.push('duration')
  return violations
}

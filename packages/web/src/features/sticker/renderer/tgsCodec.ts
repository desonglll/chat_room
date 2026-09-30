/**
 * The web host's adapter for `@tg/core`'s TGS rules: pako inflate with an incremental output
 * bound, and a plain-JSON path for Lottie that arrives uncompressed. Loaded only inside the
 * lazy render engine chunk, so pako never reaches the main bundle.
 */
import { Inflate } from 'pako'
import { TgsError, decodeTgs, isGzip, parseLottieText, type DecodedTgs, type TgsInflate } from '@tg/core/domain'

export const pakoInflate: TgsInflate = (compressed, maxOutputBytes) => {
  const chunks: Uint8Array[] = []
  let total = 0
  const inflater = new Inflate({ chunkSize: 64 * 1024 })
  inflater.onData = (chunk) => {
    total += chunk.length
    // Throwing out of onData aborts pako mid-stream: the bomb never fully materialises.
    if (total > maxOutputBytes) throw new TgsError('json-too-large')
    chunks.push(chunk)
  }
  inflater.push(compressed, true)
  if (inflater.err) throw new Error(inflater.msg || `inflate error ${inflater.err}`)
  if (!inflater.ended) throw new Error('truncated gzip stream')
  const joined = new Uint8Array(total)
  let offset = 0
  for (const chunk of chunks) {
    joined.set(chunk, offset)
    offset += chunk.length
  }
  return new TextDecoder().decode(joined)
}

const OPEN_BRACE = 0x7b

/** `.tgs` (gzip) or, for convenience, a raw Lottie `.json` document. */
export function decodeStickerBytes(bytes: Uint8Array): DecodedTgs {
  if (!isGzip(bytes) && bytes[0] === OPEN_BRACE) return parseLottieText(new TextDecoder().decode(bytes))
  return decodeTgs(bytes, pakoInflate)
}

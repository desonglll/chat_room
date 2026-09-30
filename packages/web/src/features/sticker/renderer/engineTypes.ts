/**
 * The contract between the manager and a render engine. Two engines implement it:
 * `workerEngine` (lottie in a pool of module workers, frames come back as ImageBitmaps — the
 * default) and `mainThreadEngine` (lottie on the page's thread — the fallback where workers
 * with OffscreenCanvas are unavailable). The manager cannot tell them apart.
 */
import type { LottieHeader } from '@tg/core/domain'

/** A decoded, validated, parsed sticker held by the engine until `unload(key)`. */
export interface LoadedSticker {
  key: string
  header: LottieHeader
}

export type FrameImage = HTMLCanvasElement | ImageBitmap

export interface RenderUnit {
  readonly frames: number
  readonly frameRate: number
  /** Delivery is asynchronous (a round trip), so the group asks one frame ahead. */
  readonly pipelined: boolean
  /**
   * Render `frame` and hand the image to `deliver` (synchronously or later). The image stays
   * valid until the next delivery or `destroy()`. Returns false, delivering nothing, while an
   * earlier request is still in flight — the caller simply asks again next tick.
   */
  request(frame: number, deliver: (image: FrameImage) => void): boolean
  destroy(): void
}

export interface StickerEngine {
  readonly kind: 'worker' | 'main'
  /** Inflate, validate and parse. Rejects with `TgsError` for a bad file. */
  load(key: string, bytes: Uint8Array): Promise<LoadedSticker>
  unload(key: string): void
  /** One sticker at one pixel size, positioned on frame 0. The key must be loaded. */
  createUnit(key: string, pixelSize: number): Promise<RenderUnit>
  /** PNG of frame 0. The key must be loaded. */
  snapshot(key: string, pixelSize: number): Promise<Blob>
}

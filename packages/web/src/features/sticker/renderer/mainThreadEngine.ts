/**
 * Fallback engine: lottie on the page's own thread, for browsers without module workers or
 * OffscreenCanvas. Same contract as the worker engine; every render here is paid for inside
 * the ticker's main-thread budget, so stickers lose frames sooner, but the page stays smooth.
 */
import lottie from 'lottie-web/build/player/lottie_light_canvas'
import type { DecodedTgs } from '@tg/core/domain'
import type { RenderUnit, StickerEngine } from './engineTypes'
import { frameCount, loadLottie } from './lottieCanvas'
import { decodeStickerBytes } from './tgsCodec'

function surface(pixelSize: number): { canvas: HTMLCanvasElement; context: CanvasRenderingContext2D } {
  const canvas = document.createElement('canvas')
  canvas.width = pixelSize
  canvas.height = pixelSize
  const context = canvas.getContext('2d')
  if (!context) throw new Error('2d canvas unavailable')
  return { canvas, context }
}

export function createMainThreadEngine(): StickerEngine {
  const documents = new Map<string, DecodedTgs>()
  const documentOf = (key: string) => {
    const decoded = documents.get(key)
    if (!decoded) throw new Error('sticker not loaded')
    return decoded.animation
  }
  let snapshotSurface: ReturnType<typeof surface> | null = null
  let snapshotQueue: Promise<unknown> = Promise.resolve()

  return {
    kind: 'main',
    async load(key, bytes) {
      const decoded = decodeStickerBytes(bytes)
      documents.set(key, decoded)
      return { key, header: decoded.header }
    },
    unload(key) {
      documents.delete(key)
    },
    async createUnit(key, pixelSize): Promise<RenderUnit> {
      const { canvas, context } = surface(pixelSize)
      const item = await loadLottie(lottie, context, documentOf(key))
      return {
        frames: frameCount(item),
        frameRate: item.frameRate,
        pipelined: false,
        request(frame, deliver) {
          item.goToAndStop(frame, true)
          deliver(canvas)
          return true
        },
        destroy() {
          item.destroy()
          canvas.width = 0
          canvas.height = 0
        },
      }
    },
    snapshot(key, pixelSize) {
      // One canvas reused for every snapshot, one snapshot at a time.
      const job = snapshotQueue.then(async () => {
        snapshotSurface ??= surface(pixelSize)
        const { canvas, context } = snapshotSurface
        canvas.width = pixelSize
        canvas.height = pixelSize
        const item = await loadLottie(lottie, context, documentOf(key))
        try {
          item.goToAndStop(0, true)
          return await new Promise<Blob>((resolve, reject) =>
            canvas.toBlob((blob) => (blob ? resolve(blob) : reject(new Error('snapshot encode failed'))), 'image/png'),
          )
        } finally {
          item.destroy()
        }
      })
      snapshotQueue = job.catch(() => undefined)
      return job
    },
  }
}

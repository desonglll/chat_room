/**
 * Render worker: decodes TGS, keeps parsed documents, and renders frames with lottie-web into
 * OffscreenCanvases, returning each frame as a transferred ImageBitmap (zero-copy). The page
 * thread only posts requests and draws bitmaps.
 */
import './domShim'
import type { AnimationItem, LottiePlayer } from 'lottie-web'
import { TgsError, type DecodedTgs } from '@tg/core/domain'
import { decodeStickerBytes } from '../tgsCodec'
import { frameCount, loadLottie } from '../lottieCanvas'
import type { FromWorker, ReplyValue, ToWorker } from './protocol'

interface WorkerScope {
  postMessage(message: FromWorker, transfer?: Transferable[]): void
  onmessage: ((event: MessageEvent<ToWorker>) => void) | null
}

const scope = globalThis as unknown as WorkerScope
const post = (message: FromWorker, transfer: Transferable[] = []) => scope.postMessage(message, transfer)

interface Unit {
  item: AnimationItem
  canvas: OffscreenCanvas
}

const documents = new Map<string, DecodedTgs>()
const units = new Map<number, Unit>()
// First-frame snapshots share one OffscreenCanvas, one at a time.
let snapshotCanvas: OffscreenCanvas | null = null
let snapshotQueue: Promise<unknown> = Promise.resolve()

function context(canvas: OffscreenCanvas): OffscreenCanvasRenderingContext2D {
  const context = canvas.getContext('2d')
  if (!context) throw new Error('2d OffscreenCanvas unavailable')
  return context
}

function documentOf(key: string): object {
  const decoded = documents.get(key)
  if (!decoded) throw new Error('sticker not loaded')
  return decoded.animation
}

async function reply(id: number, work: () => Promise<ReplyValue> | ReplyValue): Promise<void> {
  try {
    post({ op: 'reply', id, ok: true, value: await work() })
  } catch (error) {
    post({
      op: 'reply',
      id,
      ok: false,
      code: error instanceof TgsError ? error.code : null,
      message: error instanceof Error ? error.message : String(error),
    })
  }
}

async function start(): Promise<void> {
  // Dynamic, so the shim above is in place before lottie's module body runs.
  const lottie: LottiePlayer = (await import('lottie-web/build/player/lottie_light_canvas')).default

  const handle = (message: ToWorker): void => {
    switch (message.op) {
      case 'load':
        void reply(message.id, () => {
          const decoded = decodeStickerBytes(message.bytes)
          documents.set(message.key, decoded)
          return decoded.header
        })
        return
      case 'unload':
        documents.delete(message.key)
        return
      case 'create':
        void reply(message.id, async () => {
          const canvas = new OffscreenCanvas(message.pixelSize, message.pixelSize)
          const item = await loadLottie(lottie, context(canvas), documentOf(message.key))
          units.set(message.unit, { item, canvas })
          return { frames: frameCount(item), frameRate: item.frameRate }
        })
        return
      case 'render': {
        const unit = units.get(message.unit)
        if (!unit) return
        unit.item.goToAndStop(message.frame, true)
        const bitmap = unit.canvas.transferToImageBitmap()
        post({ op: 'frame', unit: message.unit, frame: message.frame, bitmap }, [bitmap])
        return
      }
      case 'destroy':
        units.get(message.unit)?.item.destroy()
        units.delete(message.unit)
        return
      case 'snapshot': {
        const job = snapshotQueue.then(() =>
          reply(message.id, async () => {
            snapshotCanvas ??= new OffscreenCanvas(1, 1)
            snapshotCanvas.width = message.pixelSize
            snapshotCanvas.height = message.pixelSize
            const item = await loadLottie(lottie, context(snapshotCanvas), documentOf(message.key))
            try {
              item.goToAndStop(0, true)
              return await snapshotCanvas.convertToBlob({ type: 'image/png' })
            } finally {
              item.destroy()
            }
          }),
        )
        snapshotQueue = job
      }
    }
  }

  scope.onmessage = (event) => handle(event.data)
  post({ op: 'ready' })
}

void start()

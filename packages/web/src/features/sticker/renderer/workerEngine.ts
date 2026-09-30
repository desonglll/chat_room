/**
 * Default engine: a small pool of render workers. Each sticker key is pinned to one worker
 * (the least loaded when it is first loaded), so it is parsed exactly once and all its render
 * units live next to its document. The page thread posts requests and draws ImageBitmaps.
 */
import { TgsError } from '@tg/core/domain'
import type { LottieHeader } from '@tg/core/domain'
import type { FrameImage, RenderUnit, StickerEngine } from './engineTypes'
import type { FromWorker, ReplyValue, ToWorker, UnitInfo } from './worker/protocol'

export interface WorkerLike {
  postMessage(message: ToWorker): void
  addEventListener(type: 'message', listener: (event: MessageEvent<FromWorker>) => void): void
  addEventListener(type: 'error', listener: (event: Event) => void): void
  terminate(): void
}

/** Workers beyond the page thread: ~half the cores, at least 1, at most 4. */
export function workerPoolSize(hardwareConcurrency: number): number {
  return Math.min(4, Math.max(1, Math.floor(hardwareConcurrency / 2) - 1))
}

const BOOT_TIMEOUT_MS = 5000

interface Slot {
  worker: WorkerLike
  keys: number
}

type Pending = { resolve(value: ReplyValue): void; reject(error: Error): void }
type Delivery = (frame: number, bitmap: ImageBitmap) => void

function boot(worker: WorkerLike, onMessage: (message: FromWorker) => void): Promise<WorkerLike> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('render worker did not start')), BOOT_TIMEOUT_MS)
    worker.addEventListener('error', () => {
      clearTimeout(timer)
      reject(new Error('render worker failed'))
    })
    worker.addEventListener('message', (event) => {
      if (event.data.op === 'ready') {
        clearTimeout(timer)
        resolve(worker)
      } else onMessage(event.data)
    })
  })
}

/** Resolves once every worker has booted; rejects (terminating them) if any fails. */
export async function createWorkerEngine(spawn: () => WorkerLike, size: number): Promise<StickerEngine> {
  const pending = new Map<number, Pending>()
  const deliveries = new Map<number, Delivery>()
  let nextId = 1

  const onMessage = (message: FromWorker) => {
    if (message.op === 'frame') {
      const deliver = deliveries.get(message.unit)
      if (deliver) deliver(message.frame, message.bitmap)
      else message.bitmap.close()
      return
    }
    if (message.op !== 'reply') return
    const request = pending.get(message.id)
    pending.delete(message.id)
    if (!request) return
    if (message.ok) request.resolve(message.value)
    else request.reject(message.code ? new TgsError(message.code, message.message) : new Error(message.message))
  }

  const spawned = Array.from({ length: size }, spawn)
  let slots: Slot[]
  try {
    slots = (await Promise.all(spawned.map((worker) => boot(worker, onMessage)))).map((worker) => ({ worker, keys: 0 }))
  } catch (error) {
    for (const worker of spawned) worker.terminate()
    throw error
  }
  const home = new Map<string, Slot>()

  const call = <T extends ReplyValue>(slot: Slot, message: ToWorker & { id: number }): Promise<T> =>
    new Promise<T>((resolve, reject) => {
      pending.set(message.id, { resolve: resolve as (value: ReplyValue) => void, reject })
      slot.worker.postMessage(message)
    })
  const slotOf = (key: string) => {
    const slot = home.get(key)
    if (!slot) throw new Error('sticker not loaded')
    return slot
  }

  return {
    kind: 'worker',
    async load(key, bytes) {
      let slot = home.get(key)
      if (!slot) {
        slot = slots.reduce((best, candidate) => (candidate.keys < best.keys ? candidate : best))
        slot.keys += 1
        home.set(key, slot)
      }
      try {
        const header = await call<LottieHeader>(slot, { op: 'load', id: nextId++, key, bytes })
        return { key, header }
      } catch (error) {
        home.delete(key)
        slot.keys -= 1
        throw error
      }
    },
    unload(key) {
      const slot = home.get(key)
      if (!slot) return
      home.delete(key)
      slot.keys -= 1
      slot.worker.postMessage({ op: 'unload', key })
    },
    async createUnit(key, pixelSize): Promise<RenderUnit> {
      const slot = slotOf(key)
      const unit = nextId++
      const info = await call<UnitInfo>(slot, { op: 'create', id: nextId++, unit, key, pixelSize })
      let inFlight: ((image: FrameImage) => void) | null = null
      let last: ImageBitmap | null = null
      deliveries.set(unit, (_frame, bitmap) => {
        last?.close()
        last = bitmap
        const deliver = inFlight
        inFlight = null
        deliver?.(bitmap)
      })
      return {
        frames: info.frames,
        frameRate: info.frameRate,
        pipelined: true,
        request(frame, deliver) {
          if (inFlight) return false
          inFlight = deliver
          slot.worker.postMessage({ op: 'render', unit, frame })
          return true
        },
        destroy() {
          deliveries.delete(unit)
          last?.close()
          last = null
          inFlight = null
          slot.worker.postMessage({ op: 'destroy', unit })
        },
      }
    },
    snapshot(key, pixelSize) {
      return call<Blob>(slotOf(key), { op: 'snapshot', id: nextId++, key, pixelSize })
    },
  }
}

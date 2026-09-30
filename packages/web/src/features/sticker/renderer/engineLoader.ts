/**
 * Picks and lazily loads the render engine: the worker pool where the browser can run lottie
 * in a module worker with OffscreenCanvas, otherwise lottie on the page thread. Either way
 * lottie-web and pako arrive through dynamic imports, never in the main bundle.
 */
import type { StickerEngine } from './engineTypes'

export type EnginePreference = 'auto' | 'worker' | 'main'

const loadMain = () => import('./mainThreadEngine').then((module) => module.createMainThreadEngine())

async function loadWorker(): Promise<StickerEngine> {
  const { createWorkerEngine, workerPoolSize } = await import('./workerEngine')
  const spawn = () => new Worker(new URL('./worker/renderWorker.ts', import.meta.url), { type: 'module' })
  return createWorkerEngine(spawn, workerPoolSize(navigator.hardwareConcurrency || 4))
}

const workersUsable = () =>
  typeof Worker === 'function' &&
  typeof OffscreenCanvas === 'function' &&
  typeof OffscreenCanvas.prototype.transferToImageBitmap === 'function'

const pending = new Map<EnginePreference, Promise<StickerEngine>>()

export function loadStickerEngine(preference: EnginePreference = 'auto'): Promise<StickerEngine> {
  let engine = pending.get(preference)
  if (!engine) {
    engine =
      preference === 'main' || (preference === 'auto' && !workersUsable())
        ? loadMain()
        : loadWorker().catch((error: unknown) => {
            if (preference === 'worker') throw error
            return loadMain()
          })
    // A failed chunk load (flaky network) is retried by the next sticker, not cached forever.
    engine.catch(() => pending.delete(preference))
    pending.set(preference, engine)
  }
  return engine
}

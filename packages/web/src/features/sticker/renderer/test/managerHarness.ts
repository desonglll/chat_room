/** Fakes for every injected service of the sticker manager. */
import type { LottieHeader } from '@tg/core/domain'
import type { FrameImage, RenderUnit, StickerEngine } from '../engineTypes'
import type { TickerHost } from '../frameTicker'
import { createStickerRenderManager, type ManagerPolicy, type StickerViewState } from '../stickerManager'

export const header = (frames = 60): LottieHeader => ({
  version: '5.7.4',
  width: 512,
  height: 512,
  frameRate: 60,
  inPoint: 0,
  outPoint: frames,
  frames,
  durationMs: (frames / 60) * 1000,
  layerCount: 1,
})

export function harness(policy: Partial<ManagerPolicy> = {}, frames = 60) {
  const calls = { load: [] as string[], unload: [] as string[], units: 0, destroyed: 0, renders: 0, snapshots: 0 }
  const engine: StickerEngine = {
    kind: 'main',
    async load(key, bytes) {
      calls.load.push(key)
      if (bytes[0] === 0) throw new Error('bad sticker')
      return { key, header: header(frames) }
    },
    unload: (key) => void calls.unload.push(key),
    async createUnit(): Promise<RenderUnit> {
      calls.units += 1
      return {
        frames,
        frameRate: 60,
        pipelined: false,
        request(frame, deliver) {
          calls.renders += 1
          deliver({ frame } as unknown as FrameImage)
          return true
        },
        destroy: () => void (calls.destroyed += 1),
      }
    },
    async snapshot() {
      calls.snapshots += 1
      return new Blob(['png'])
    },
  }
  const signals = { hidden: false, reducedMotion: false, listeners: new Set<() => void>() }
  const visibility = new Map<object, (visible: boolean) => void>()
  let now = 0
  let queued: ((now: number) => void) | null = null
  const tickerHost: TickerHost = {
    requestFrame: (callback) => {
      queued = callback
      return 1
    },
    cancelFrame: () => {
      queued = null
    },
    now: () => now,
  }
  const blits: Array<{ canvas: object; frame: number }> = []
  let urls = 0
  const manager = createStickerRenderManager(
    {
      loadEngine: async () => engine,
      fetchBytes: async (url) => new Uint8Array([url.includes('bad') ? 0 : 1]),
      objectUrls: { create: () => `blob:poster-${++urls}`, revoke: () => undefined },
      signals: {
        hidden: () => signals.hidden,
        reducedMotion: () => signals.reducedMotion,
        pixelRatio: () => 2,
        subscribe(listener) {
          signals.listeners.add(listener)
          return () => signals.listeners.delete(listener)
        },
      },
      viewport: {
        observe(element, onChange) {
          visibility.set(element, onChange)
          return () => visibility.delete(element)
        },
      },
      tickerHost,
      blit: (canvas, image) => blits.push({ canvas, frame: (image as unknown as { frame: number }).frame }),
    },
    { maxGroups: 24, budgetMs: 8, fpsCap: () => 60, idleReleaseMs: 50, ...policy },
  )

  const flush = async () => {
    for (let round = 0; round < 10; round += 1) await new Promise((resolve) => setTimeout(resolve, 0))
  }
  const tick = (ms = 1000 / 60) => {
    now += ms
    const callback = queued
    queued = null
    callback?.(now)
  }
  const setSignal = async (patch: Partial<Pick<typeof signals, 'hidden' | 'reducedMotion'>>) => {
    Object.assign(signals, patch)
    for (const listener of signals.listeners) listener()
    await flush()
  }

  const mount = (url: string, options: { loop?: boolean; autoplay?: boolean; size?: number } = {}) => {
    const element = {}
    const canvas = { width: 0, height: 0 } as HTMLCanvasElement
    const states: StickerViewState[] = []
    const errors: unknown[] = []
    const view = manager.attach({
      element: element as Element,
      canvas,
      source: { url },
      size: options.size ?? 100,
      loop: options.loop ?? true,
      autoplay: options.autoplay ?? true,
      onState: (state) => states.push(state),
      onError: (error) => errors.push(error),
    })
    const setVisible = async (visible: boolean) => {
      visibility.get(element)?.(visible)
      await flush()
    }
    return { view, canvas, states, errors, setVisible, phase: () => states.at(-1)?.phase }
  }

  return { manager, calls, mount, tick, flush, setSignal, blits, ticking: () => queued !== null }
}

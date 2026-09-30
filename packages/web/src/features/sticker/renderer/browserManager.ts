/**
 * The page-wide manager wired to real browser services, created on first use (importing the
 * renderer never touches `window`, so server-side markup tests stay DOM-free).
 */
import { loadStickerEngine, type EnginePreference } from './engineLoader'
import type { FrameImage } from './engineTypes'
import { browserPageSignals } from './pageSignals'
import { fetchStickerBytes } from './stickerSource'
import { createStickerRenderManager, type ManagerPolicy, type StickerRenderManager } from './stickerManager'
import { browserViewportWatcher } from './viewportWatcher'

/**
 * Defaults justified by the benchmark in docs/devlog/TG-301.md:
 * - 24 groups: the acceptance target is 20 distinct stickers on screen; a little headroom so a
 *   sticker scrolling in does not push one that is already playing back to static.
 * - 8 ms budget: half a 60 Hz frame, leaving the other half for layout, paint and the app.
 * - 60 fps for a handful of stickers (a large preview looks noticeably smoother), 30 fps
 *   beyond that — the same trade Telegram's own clients make under load.
 */
export const DEFAULT_POLICY: ManagerPolicy = {
  maxGroups: 24,
  budgetMs: 8,
  fpsCap: (playing) => (playing <= 4 ? 60 : 30),
  idleReleaseMs: 10_000,
}

const contexts = new WeakMap<HTMLCanvasElement, CanvasRenderingContext2D | null>()

function blit(target: HTMLCanvasElement, image: FrameImage): void {
  let context = contexts.get(target)
  if (context === undefined) {
    context = target.getContext('2d')
    contexts.set(target, context)
  }
  if (!context) return
  context.clearRect(0, 0, target.width, target.height)
  context.drawImage(image, 0, 0, target.width, target.height)
}

let shared: StickerRenderManager | null = null

/** The page-wide manager every `<AnimatedSticker>` uses unless handed another. */
export function stickerRenderManager(): StickerRenderManager {
  shared ??= createBrowserStickerManager()
  return shared
}

/** A separately configured manager — for the benchmark, which compares engines and policies. */
export function createBrowserStickerManager(
  engine: EnginePreference = 'auto',
  policy: ManagerPolicy = DEFAULT_POLICY,
): StickerRenderManager {
  return createStickerRenderManager(
    {
      loadEngine: () => loadStickerEngine(engine),
      fetchBytes: fetchStickerBytes,
      objectUrls: { create: (blob) => URL.createObjectURL(blob), revoke: (url) => URL.revokeObjectURL(url) },
      signals: browserPageSignals(),
      viewport: browserViewportWatcher(),
      tickerHost: {
        requestFrame: (callback) => requestAnimationFrame(callback),
        cancelFrame: (handle) => cancelAnimationFrame(handle),
        now: () => performance.now(),
      },
      blit,
    },
    policy,
  )
}

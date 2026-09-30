/** The manager's public contract; implementation in `stickerManager.ts`. */
import type { StickerEngine } from './engineTypes'
import type { TickerHost, TickerStats } from './frameTicker'
import type { PageSignals } from './pageSignals'
import type { MotionView, MotionViewOptions } from './motionViews'
import type { Blit } from './stickerGroup'
import type { StickerSource } from './stickerSource'
import type { ViewportWatcher } from './viewportWatcher'

export type StickerPhase = 'loading' | 'static' | 'live' | 'error'

export interface StickerViewState {
  phase: StickerPhase
  /** Object URL of the first frame, once rendered. */
  poster: string | null
}

export interface StickerViewOptions {
  /** Observed for viewport intersection. */
  element: Element
  /** Live frames are copied here; the manager sizes it. */
  canvas: HTMLCanvasElement
  source: StickerSource
  /** CSS pixels (square). */
  size: number
  loop: boolean
  autoplay: boolean
  onState(state: StickerViewState): void
  onError?(error: unknown): void
}

export interface StickerView {
  /** Play from the first frame (tap-to-play, or replay a finished non-looping sticker). */
  replay(): void
  destroy(): void
}

export interface ManagerServices {
  loadEngine(): Promise<StickerEngine>
  fetchBytes(url: string): Promise<Uint8Array>
  objectUrls: { create(blob: Blob): string; revoke(url: string): void }
  signals: PageSignals
  viewport: ViewportWatcher
  tickerHost: TickerHost
  blit: Blit
}

export interface ManagerPolicy {
  /** Distinct render groups allowed to animate at once. */
  maxGroups: number
  /** Main-thread milliseconds per animation frame spent rendering stickers. */
  budgetMs: number
  fpsCap(playingGroups: number): number
  /** A paused group's render unit is released after this long. */
  idleReleaseMs: number
}

export interface ManagerStats extends TickerStats {
  views: number
  liveViews: number
  playingViews: number
  groups: number
  runningGroups: number
  parsed: number
  /** WebM (browser-animated) stickers registered, and how many of them may play. */
  motionViews: number
  playingMotionViews: number
}

export interface StickerRenderManager {
  attach(options: StickerViewOptions): StickerView
  /** A sticker the browser animates (WebM): same viewport/hidden/reduced-motion policy and cap. */
  attachMotion(options: MotionViewOptions): MotionView
  stats(): ManagerStats
}

/**
 * TG-301 animated-sticker renderer. Public surface — see docs/devlog/TG-301.md "Frozen
 * interface". Importing this module does not load lottie-web or pako; the first sticker does.
 */

export { AnimatedSticker, DEFAULT_STICKER_SIZE, type AnimatedStickerProps } from './AnimatedSticker'
export { stickerRenderManager, createBrowserStickerManager, DEFAULT_POLICY } from './browserManager'
export type { EnginePreference } from './engineLoader'
export {
  createStickerRenderManager,
  type ManagerPolicy,
  type ManagerServices,
  type ManagerStats,
  type StickerPhase,
  type StickerRenderManager,
  type StickerView,
  type StickerViewOptions,
  type StickerViewState,
} from './stickerManager'
export type { StickerSource } from './stickerSource'
export type { PageSignals } from './pageSignals'
export type { ViewportWatcher } from './viewportWatcher'

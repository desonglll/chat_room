/**
 * Sticker renderer. Public surface — see docs/devlog/TG-301.md and TG-306.md "Frozen
 * interface". Importing this module does not load lottie-web or pako; the first TGS does.
 *
 * `<Sticker sticker={…}>` is the entry for every format (TGS / WebP / WebM); the TG-301
 * exports below it stay for callers that already hold TGS bytes.
 */
export { Sticker, type StickerProps } from './Sticker'
export {
  sniffStickerFormat,
  stickerFromMessage,
  stickerUrl,
  wireStickerFormat,
  type StickerDescriptor,
  type StickerFormat,
} from './stickerFormat'
export { detectWebmStickerSupport, setWebmStickerSupport, webmStickerSupported } from './webmSupport'
export type { MotionView, MotionViewOptions } from './motionViews'

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

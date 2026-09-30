/**
 * TG-303 public surface — see docs/devlog/TG-303.md "Frozen interface".
 *
 * - `LazyMediaPanel` — the emoji / 贴纸 / GIF panel the composer opens (code-split).
 * - `registerMediaPanelTab` — how TG-305 plugs the GIF tab in.
 * - `StickerSuggestions` — the emoji → sticker strip above the input.
 * - `StickerOverlays` — set preview + management page host, mounted once by the shell.
 * - `StickerSetLinkRoute` — the `/addstickers/:shortName` route element.
 * - `stickerLibrary()` — the use cases (load, install, reorder, favorite, send).
 * - `./register` (side effect) — sticker messages in the bubble registry.
 */
export { LazyMediaPanel } from './LazyMediaPanel'
export type { MediaPanelProps } from './panel/MediaPanel'
export { registerMediaPanelTab } from './panel/mediaPanelTabs'
export type { MediaPanelTab, MediaPanelTabContext } from './panel/mediaPanelTabs'
export { StickerSuggestions } from './suggest/StickerSuggestions'
export type { StickerSuggestionsProps } from './suggest/StickerSuggestions'
export { StickerOverlays } from './StickerOverlays'
export { StickerSetLinkRoute } from './StickerSetLinkRoute'
export { openStickerSet, openStickerSettings } from './overlayStore'
export { stickerLibrary, createStickerLibrary } from './stickerLibrary'
export type { StickerLibrary, SendStickerInput } from './stickerLibrary'
export { StickerView } from './StickerView'

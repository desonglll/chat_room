/**
 * The workspace-level sticker overlays: the set management page and, above it, a set
 * preview (by short name — from the panel, a sticker bubble, the management page or an
 * `/addstickers/:name` link). They are hosted by `<StickerOverlays>` in the shell, not
 * inside the composer popover, so they never depend on the popover staying open.
 */
import { createStore } from 'zustand/vanilla'

export interface StickerOverlayState {
  settingsOpen: boolean
  /** Short name of the set being previewed, above the settings page when both are open. */
  setShortName: string | null
}

export const stickerOverlayStore = createStore<StickerOverlayState>()(() => ({
  settingsOpen: false,
  setShortName: null,
}))

export const openStickerSet = (shortName: string) =>
  stickerOverlayStore.setState({ setShortName: shortName.trim().toLowerCase() })

export const closeStickerSet = () => stickerOverlayStore.setState({ setShortName: null })

export const openStickerSettings = () => stickerOverlayStore.setState({ settingsOpen: true })

export const closeStickerSettings = () => stickerOverlayStore.setState({ settingsOpen: false })

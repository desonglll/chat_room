/**
 * TG-105 public surface. The lead mounts `<MediaViewer />` once at the app root and binds the
 * bubble's `actions.onOpenMedia` to
 * `(attachmentId) => openMediaViewer({ chatId, attachmentId })`. See docs/devlog/TG-105.md
 * "Frozen interface".
 */
export { MediaViewer } from './MediaViewer'
export { openMediaViewer, closeMediaViewer, mediaViewerStore } from './mediaViewerStore'
export type { MediaViewerRequest, OpenMediaViewerInput } from './mediaViewerStore'
export type { MediaViewerActions, MediaViewerProps } from './types'
export type { MediaItem, MediaKind } from './mediaItem'
export type { FetchMediaPage, MediaPage } from './mediaPager'
export { createChatMediaFetcher, toMediaPage } from './chatMediaSource'
export type { Rect } from './zoomGeometry'

/** Public props of the viewer (TG-105 frozen interface, docs/devlog/TG-105.md). */
import type { MediaItem } from './mediaItem'
import type { FetchMediaPage } from './mediaPager'

/**
 * What the host lets the viewer do with a medium. Every member is optional; an absent
 * callback hides its button (download falls back to a plain `<a download>`).
 */
export interface MediaViewerActions {
  /** Replace the built-in `<a download>` (e.g. to go through a native save dialog). */
  onDownload?: ((item: MediaItem) => void) | undefined
  /** Open the forward picker for `item.messageId`. The viewer stays open underneath. */
  onForward?: ((item: MediaItem) => void) | undefined
  /**
   * Delete (recall) `item.messageId`, including any confirmation. Resolve `true` when it was
   * deleted: the viewer then drops the item and moves to a neighbour, or closes if none.
   */
  onDelete?: ((item: MediaItem) => Promise<boolean> | boolean) | undefined
  /** Whether the viewer may offer delete for `item`; defaults to true when `onDelete` is set. */
  canDelete?: ((item: MediaItem) => boolean) | undefined
}

export interface MediaViewerProps {
  actions?: MediaViewerActions | undefined
  /**
   * The page source for a Chat's media. Defaults to `/api/chats/:id/files` with the session
   * token (`chatMediaSource.ts`). Tests and fixtures inject a fake.
   */
  createFetcher?: ((chatId: string) => FetchMediaPage) | undefined
}

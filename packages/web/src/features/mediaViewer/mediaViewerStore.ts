/**
 * Whether the media viewer is open and on what. One app-wide instance: the lead mounts one
 * `<MediaViewer />` at the root and binds the bubble's `actions.onOpenMedia` to
 * `openMediaViewer`. Feature-local (not `@tg/core/stores`) because it is pure UI state.
 */
import { createStore } from 'zustand/vanilla'
import type { Rect } from './zoomGeometry'

export interface MediaViewerRequest {
  /** Increments per open, so reopening the same attachment remounts a fresh viewer. */
  openId: number
  chatId: string
  attachmentId: string
  /**
   * Where the thumbnail is on screen, for the shared-element zoom. Optional: without it the
   * viewer finds the thumbnail in the DOM by its URL, and fades when there is none.
   */
  sourceRect: Rect | null
}

export interface OpenMediaViewerInput {
  chatId: string
  attachmentId: string
  sourceRect?: Rect | null | undefined
}

export interface MediaViewerState {
  request: MediaViewerRequest | null
  open(input: OpenMediaViewerInput): void
  close(): void
}

export const createMediaViewerStore = () => {
  let opens = 0
  return createStore<MediaViewerState>()((set) => ({
    request: null,
    open: ({ chatId, attachmentId, sourceRect }) => {
      opens += 1
      set({ request: { openId: opens, chatId, attachmentId, sourceRect: sourceRect ?? null } })
    },
    close: () => set({ request: null }),
  }))
}

export type MediaViewerStore = ReturnType<typeof createMediaViewerStore>

export const mediaViewerStore = createMediaViewerStore()

/** Open the viewer on one attachment of one Chat. Safe to call while it is already open. */
export function openMediaViewer(input: OpenMediaViewerInput): void {
  mediaViewerStore.getState().open(input)
}

/** Close immediately, without the exit animation (e.g. the Chat was left). */
export function closeMediaViewer(): void {
  mediaViewerStore.getState().close()
}

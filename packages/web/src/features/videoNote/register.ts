/**
 * Plugs the round video bubble into every bubble through TG-103's content registry (TG-402).
 * Imported once for its side effect (`src/main.tsx`). A message is a video note exactly when
 * it carries `video_note` (the server sets it only with `media_kind: "video_note"` and a
 * visible attachment).
 *
 * Frame `bare` (Telegram draws the circle with no bubble) with the time overlaid, like a
 * sticker. Priority 41: above the generic video kind (20) — which would otherwise draw a
 * rectangular player — and voice (40); below polls (50) and the recalled placeholder (100).
 */
import type { BroadcastMessage } from '@tg/core'
import { registerMessageContent } from '../message'
import { VideoNoteContent } from './VideoNoteContent'

export const VIDEO_NOTE_CONTENT_PRIORITY = 41

export const isVideoNoteContent = (message: BroadcastMessage): boolean =>
  message.video_note != null && message.attachment !== null

export const unregisterVideoNoteContent = registerMessageContent('video_note', VideoNoteContent, {
  match: isVideoNoteContent,
  frame: 'bare',
  metaPlacement: 'overlay',
  priority: VIDEO_NOTE_CONTENT_PRIORITY,
})

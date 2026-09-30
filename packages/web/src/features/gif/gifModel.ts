/**
 * What makes a message a GIF, and how its file plays. Pure — shared by the bubble, the
 * panel and the "保存 GIF" menu row.
 *
 * A GIF is stored as whatever was uploaded (TG-305 devlog Decisions: the server has no
 * transcoder): a silent MP4/WebM plays in a muted looping `<video>`, a real `.gif` falls
 * back to `<img>`.
 */
import type { BroadcastMessage } from '@tg/core'

export const GIF_MEDIA_KIND = 'gif'

export type GifRenderMode = 'video' | 'image'

export function gifRenderMode(mimeType: string): GifRenderMode {
  return mimeType.toLowerCase().startsWith('video/') ? 'video' : 'image'
}

/**
 * Sent as a GIF (`media_kind: 'gif'`), or a real GIF file sent as an ordinary attachment.
 * A sensitive attachment is left to the photo kind, which has the veil.
 */
export function isGifMessage(message: BroadcastMessage): boolean {
  const attachment = message.attachment
  if (attachment === null || message.recalled_at !== null) return false
  if (message.media_kind === GIF_MEDIA_KIND) return true
  return !attachment.is_sensitive && attachment.mime_type.toLowerCase() === 'image/gif'
}

/**
 * Recognising a sticker message and sizing it, as pure functions over the wire message.
 *
 * TG-302 marks a sticker message with `media_kind: 'sticker'` plus a `sticker` block. A
 * message that lost those fields (a forwarded sticker — TG-302 residual risk — or an old
 * client path) is still recognised by the TGS MIME type, which nothing else uses.
 */
import type { BroadcastMessage, MessageSticker, StickerFormat } from '@tg/core'

export const TGS_MIME = 'application/x-tgsticker'
/** Telegram desktop draws a sticker message in a box of about this many CSS pixels. */
export const STICKER_MESSAGE_SIZE = 192

export function isStickerMessage(message: BroadcastMessage): boolean {
  if (message.attachment === null) return false
  return message.media_kind === 'sticker' || message.attachment.mime_type === TGS_MIME
}

export interface StickerMessageView {
  src: string
  format: StickerFormat
  emoji: string
  setShortName: string | null
  width: number
  height: number
}

function formatOf(mime: string): StickerFormat {
  if (mime === TGS_MIME) return 'tgs'
  if (mime === 'video/webm') return 'webm'
  return 'webp'
}

/** Fits the sticker's own aspect ratio into the square message box. */
export function fitSticker(
  width: number,
  height: number,
  box = STICKER_MESSAGE_SIZE,
): { width: number; height: number } {
  if (!(width > 0) || !(height > 0)) return { width: box, height: box }
  const scale = box / Math.max(width, height)
  return { width: Math.round(width * scale), height: Math.round(height * scale) }
}

export function stickerMessageView(message: BroadcastMessage): StickerMessageView | null {
  const attachment = message.attachment
  if (attachment === null) return null
  const sticker: MessageSticker | undefined = message.sticker
  const size = fitSticker(sticker?.width ?? 512, sticker?.height ?? 512)
  return {
    src: attachment.download_url,
    format: sticker?.format ?? formatOf(attachment.mime_type),
    emoji: sticker?.emoji ?? '',
    setShortName: sticker?.set_short_name ?? null,
    ...size,
  }
}

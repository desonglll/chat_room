/**
 * Which renderer a sticker needs. The wire says so (TG-302: `format` on catalogue stickers and
 * on a message's `sticker` block, `mime_type` on both); bytes are sniffed only when the wire
 * is silent or says something this client does not know.
 */

export type StickerFormat = 'tgs' | 'webp' | 'webm'

/**
 * Structural view of TG-302's wire shapes — a catalogue `Sticker` fits as is; a sticker
 * message is `{ ...message.sticker, download_url: message.attachment.download_url,
 * mime_type: message.attachment.mime_type }` (see `stickerFromMessage`).
 */
export interface StickerDescriptor {
  format?: string | null
  mime_type?: string | null
  /** Catalogue capability URL (`/api/stickers/:id/file?key=`). */
  file_url?: string | null
  /** Per-message attachment capability URL; wins over `file_url` when both are present. */
  download_url?: string | null
  /** Optional static thumbnail; also the WebM fallback. Not on TG-302's wire yet. */
  thumbnail_url?: string | null
  emoji?: string | null
  width?: number | null
  height?: number | null
}

interface MessageLike {
  sticker?: { format?: string | null; emoji?: string | null; width?: number | null; height?: number | null } | null
  attachment?: { download_url?: string | null; mime_type?: string | null } | null
}

/** A sticker message (history `StoredMessage` or WS broadcast) as a descriptor, or null. */
export function stickerFromMessage(message: MessageLike): StickerDescriptor | null {
  if (!message.sticker || !message.attachment?.download_url) return null
  return {
    ...message.sticker,
    download_url: message.attachment.download_url,
    mime_type: message.attachment.mime_type ?? null,
  }
}

const MIME_FORMATS: Record<string, StickerFormat> = {
  'application/x-tgsticker': 'tgs',
  'application/json': 'tgs',
  'image/webp': 'webp',
  'video/webm': 'webm',
}

/** The format the wire declares, or null when it must be sniffed. */
export function wireStickerFormat(sticker: StickerDescriptor): StickerFormat | null {
  const format = sticker.format?.toLowerCase()
  if (format === 'tgs' || format === 'webp' || format === 'webm') return format
  const mime = sticker.mime_type?.split(';')[0]?.trim().toLowerCase()
  return (mime && MIME_FORMATS[mime]) || null
}

export function stickerUrl(sticker: StickerDescriptor): string | null {
  return sticker.download_url || sticker.file_url || null
}

const startsWith = (bytes: Uint8Array, offset: number, magic: readonly number[]) =>
  bytes.length >= offset + magic.length && magic.every((byte, index) => bytes[offset + index] === byte)

/**
 * Magic-byte fallback: gzip → TGS, `RIFF….WEBP` → WebP, EBML → WebM, a JSON object → raw
 * Lottie (rendered by the TGS path). Anything else is null.
 */
export function sniffStickerFormat(bytes: Uint8Array): StickerFormat | null {
  if (startsWith(bytes, 0, [0x1f, 0x8b])) return 'tgs'
  if (startsWith(bytes, 0, [0x52, 0x49, 0x46, 0x46]) && startsWith(bytes, 8, [0x57, 0x45, 0x42, 0x50])) return 'webp'
  if (startsWith(bytes, 0, [0x1a, 0x45, 0xdf, 0xa3])) return 'webm'
  const start = startsWith(bytes, 0, [0xef, 0xbb, 0xbf]) ? 3 : 0
  for (const byte of bytes.subarray(start, start + 64)) {
    if (byte === 0x20 || byte === 0x09 || byte === 0x0a || byte === 0x0d) continue
    return byte === 0x7b ? 'tgs' : null
  }
  return null
}

export const STICKER_MIME: Record<StickerFormat, string> = {
  tgs: 'application/x-tgsticker',
  webp: 'image/webp',
  webm: 'video/webm',
}

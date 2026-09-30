/** Pure classification of a message's attachment, shared by the built-in media kinds. */
import type { Attachment, BroadcastMessage } from '@tg/core'

export type AttachmentKind = 'image' | 'video' | 'file'

export function attachmentKind(attachment: Attachment): AttachmentKind {
  const mime = attachment.mime_type.toLowerCase()
  // SVG is an image type but a script-capable document; it renders as a file card.
  if (mime.startsWith('image/') && mime !== 'image/svg+xml') return 'image'
  if (mime.startsWith('video/')) return 'video'
  return 'file'
}

export function hasAttachmentOf(message: BroadcastMessage, kind: AttachmentKind): boolean {
  return message.attachment !== null && attachmentKind(message.attachment) === kind
}

export function hasCaption(message: BroadcastMessage): boolean {
  return message.content.trim() !== ''
}

const UNITS = ['B', 'KB', 'MB', 'GB'] as const

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return ''
  let value = bytes
  let unit = 0
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024
    unit += 1
  }
  const digits = unit === 0 || value >= 100 ? 0 : 1
  return `${value.toFixed(digits)} ${UNITS[unit]}`
}

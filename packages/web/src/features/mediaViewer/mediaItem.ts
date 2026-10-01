/**
 * The viewer's unit: one photo or video of one Chat, from either source — a loaded
 * `BroadcastMessage` (the chat timeline) or a `ChatFileItem` (the `/files` listing). Pure.
 */
import type { Attachment, ChatFileItem, DisplayMessage } from '@tg/core'
import { attachmentKind } from '../message/content/attachmentKind'

export type MediaKind = 'image' | 'video'

export interface MediaItem {
  attachmentId: string
  messageId: string
  kind: MediaKind
  url: string
  fileName: string
  mimeType: string
  sizeBytes: number
  isSensitive: boolean
  sender: string
  senderId: string | null
  /** Server `created_at`; ordering key together with `messageId`. */
  createdAt: string
  /** Message text shown under the media. `null` when the source does not carry it (`/files`). */
  caption: string | null
}

/** Photos and videos only; SVG and every other type stay file cards (TG-103's rule). */
export function viewerKind(attachment: Attachment): MediaKind | null {
  const kind = attachmentKind(attachment)
  return kind === 'file' ? null : kind
}

function fromAttachment(
  attachment: Attachment,
  message: { messageId: string; sender: string; senderId: string | null; createdAt: string; caption: string | null },
): MediaItem | null {
  const kind = viewerKind(attachment)
  if (kind === null) return null
  return {
    attachmentId: attachment.id,
    kind,
    url: attachment.download_url,
    fileName: attachment.file_name,
    mimeType: attachment.mime_type,
    sizeBytes: attachment.size_bytes,
    isSensitive: attachment.is_sensitive,
    ...message,
  }
}

/** The viewable media of a loaded timeline, oldest first. Recalled messages are gone. */
export function mediaFromMessages(messages: readonly DisplayMessage[]): MediaItem[] {
  const items: MediaItem[] = []
  for (const message of messages) {
    if (message.type !== 'broadcast' || message.attachment === null || message.recalled_at) continue
    const item = fromAttachment(message.attachment, {
      messageId: message.message_id,
      sender: message.sender,
      senderId: message.sender_id,
      createdAt: message.timestamp,
      caption: message.content,
    })
    if (item) items.push(item)
  }
  return sortItems(items)
}

export function mediaFromFileItem(file: ChatFileItem): MediaItem | null {
  return fromAttachment(file.attachment, {
    messageId: file.message_id,
    sender: file.sender,
    senderId: file.sender_id,
    createdAt: file.created_at,
    caption: null,
  })
}

/** Nanoseconds below the millisecond, from the RFC 3339 fraction (`Date.parse` drops them). */
function subMillisecondNanos(iso: string): number {
  const fraction = /\.(\d+)/.exec(iso)?.[1] ?? ''
  return Number(fraction.slice(3, 9).padEnd(6, '0'))
}

/**
 * Chronological order. Parsed, not string-compared: the server's RFC 3339 output trims its
 * fractional seconds, so two timestamps of different lengths do not sort lexically. Sub-
 * millisecond digits break ties: the items of one album are written 1 µs apart (TG-1202).
 */
export function compareItems(a: MediaItem, b: MediaItem): number {
  const delta = Date.parse(a.createdAt) - Date.parse(b.createdAt)
  if (delta !== 0 && Number.isFinite(delta)) return delta
  const fine = subMillisecondNanos(a.createdAt) - subMillisecondNanos(b.createdAt)
  if (fine !== 0) return fine
  return a.messageId < b.messageId ? -1 : a.messageId > b.messageId ? 1 : 0
}

function sortItems(items: MediaItem[]): MediaItem[] {
  return items.sort(compareItems)
}

/**
 * Union by attachment id, oldest first. On a collision the richer copy wins: a timeline
 * item carries the caption a `/files` item does not.
 */
export function mergeItems(existing: readonly MediaItem[], incoming: readonly MediaItem[]): MediaItem[] {
  const byId = new Map<string, MediaItem>()
  for (const item of existing) byId.set(item.attachmentId, item)
  for (const item of incoming) {
    const known = byId.get(item.attachmentId)
    byId.set(item.attachmentId, known && known.caption !== null && item.caption === null ? known : item)
  }
  return sortItems([...byId.values()])
}

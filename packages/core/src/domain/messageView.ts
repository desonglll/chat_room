/**
 * Client-side view types over the wire contract.
 *
 * `types/` mirrors the Rust wire exactly; the fields here (`delivery_state`, `motion`,
 * upload placeholders, locally keyed system rows) exist only inside the client, so they
 * live in domain/. `BroadcastMessage` is the wire `broadcast` frame plus those view fields —
 * spreading a received frame into the list is still type-correct.
 *
 * Optional view fields are `?: X | undefined` because the workspace compiles with
 * `exactOptionalPropertyTypes` and reconciliation deliberately writes `undefined` to clear.
 */
import type { BroadcastFrame, StoredMessage } from '../types'

export type MessageMotion = 'none' | 'incoming' | 'outgoing' | 'system'

export type DeliveryState = 'sending' | 'sent' | 'failed'

export type ChatConnectionStatus = 'idle' | 'connecting' | 'online' | 'offline' | 'failed'

export type UploadPhase = 'queued' | 'hashing' | 'uploading' | 'deduplicating' | 'finalizing'

export type UploadTaskStatus = 'pending' | 'failed'

export interface BroadcastMessage extends BroadcastFrame {
  delivery_state?: DeliveryState | undefined
  motion?: MessageMotion | undefined
}

/** A locally rendered system row (join/leave/rename …); `key` is client-generated. */
export interface SystemMessage {
  type: 'system'
  key: string
  content: string
  motion?: MessageMotion | undefined
}

/** A local upload placeholder row shown while an attachment is hashing/uploading. */
export interface UploadMessage {
  type: 'upload'
  key: string
  room_id: string
  file_name: string
  mime_type: string
  size_bytes: number
  preview_url: string
  is_sensitive: boolean
  content: string
  phase: UploadPhase
  processed_bytes: number
  total_bytes: number
  status: UploadTaskStatus
  error: string
  timestamp: string
}

export type DisplayMessage = BroadcastMessage | SystemMessage | UploadMessage

/** REST → WS shape bridge, used by history loads and reconnect catch-up. */
export function storedMessageToBroadcast(message: StoredMessage): BroadcastMessage {
  return {
    type: 'broadcast',
    message_id: message.id,
    // Mirror the wire's omitted-when-None so reconcileOptimisticMessage can match a
    // pending send whose ack arrives via reconnect catch-up instead of the live frame.
    ...(message.client_message_id ? { client_message_id: message.client_message_id } : {}),
    sender_id: message.sender_id,
    sender: message.sender,
    sender_avatar: message.sender_avatar,
    content: message.content,
    attachment: message.attachment,
    reply_to: message.reply_to,
    recalled_at: message.recalled_at,
    edited_at: message.edited_at,
    timestamp: message.created_at,
    favorite_id: message.favorite_id,
    forwarded_from: message.forwarded_from,
    reactions: message.reactions || [],
    ...(message.poll ? { poll: message.poll } : {}),
    ...(message.entities?.length ? { entities: message.entities } : {}),
    ...(message.media_kind ? { media_kind: message.media_kind } : {}),
    ...(message.sticker ? { sticker: message.sticker } : {}),
    ...(message.voice ? { voice: message.voice } : {}),
  }
}

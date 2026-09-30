/**
 * Mirrors of the message-domain REST contract (`src/models.rs` / TG-004 devlog §5).
 *
 * `room_id` keeps its spelling on purpose: TG-004 "Frozen non-rename 1" / decision D-003
 * froze every foreign key to `chats(id)` as `room_id` until a dedicated task renames them
 * all at once. Do not "fix" it here — this file mirrors the wire.
 */

export interface Attachment {
  id: string
  file_name: string
  mime_type: string
  size_bytes: number
  download_url: string
  is_sensitive: boolean
}

export interface ReplyPreview {
  message_id: string
  sender: string
  content: string
  attachment_file_name: string | null
  recalled: boolean
}

export interface ForwardedFrom {
  sender: string
  /** Frozen spelling — pre-rename clients string-match it (CONTEXT.md "Room"). */
  room_name: string
}

export interface MessageReaction {
  emoji: string
  user_ids: string[]
}

export interface ReadReceipt {
  user_id: string
  username: string
  message_id: string
}

/** One persisted message as `GET /api/chats/:id/messages` returns it. */
export interface StoredMessage {
  id: string
  room_id: string
  /** Always serialised by the server (null or a UUID) — the optimistic-send reconcile key. */
  client_message_id: string | null
  sender_id: string | null
  sender: string
  sender_avatar: string
  content: string
  attachment: Attachment | null
  reply_to: ReplyPreview | null
  recalled_at: string | null
  edited_at: string | null
  created_at: string
  favorite_id: string | null
  forwarded_from: ForwardedFrom | null
  reactions: MessageReaction[]
}

/**
 * The keyset pagination cursor (`MessageCursor { created_at, id }` server-side,
 * architecture.md §5.3). High-churn fields (views, votes) never enter it.
 */
export interface MessageCursor {
  created_at: string
  id: string
}

export interface ForwardResult {
  message_id: string
  target_room_id: string
  forwarded_message_id: string | null
  skipped_reason: string | null
}

export interface ChatFileItem {
  message_id: string
  sender_id: string | null
  sender: string
  sender_avatar: string
  created_at: string
  attachment: Attachment
}

export interface ChatFilePage {
  items: ChatFileItem[]
  next_before: string | null
}

export interface ChatPin {
  message: StoredMessage
  pinned_by: string
  pinned_at: string
}

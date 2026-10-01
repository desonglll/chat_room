/**
 * Mirrors of the message-domain REST contract (`src/models.rs` / TG-004 devlog §5).
 *
 * `room_id` keeps its spelling on purpose: TG-004 "Frozen non-rename 1" / decision D-003
 * froze every foreign key to `chats(id)` as `room_id` until a dedicated task renames them
 * all at once. Do not "fix" it here — this file mirrors the wire.
 */

import type { PollState } from './realtime'

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
  /** TG-409: the snippet the sender quoted; omitted for an ordinary reply. */
  quote?: ReplyQuote
  /** TG-409: the original changed after it was quoted. */
  quote_modified?: boolean
  /** TG-409: set for a reply to a message in another chat (which the viewer may not open). */
  chat_id?: string
  chat_title?: string
}

/** TG-410: a shared account, snapshotted when the card was sent (`user_id` null once deleted). */
export interface ContactCard {
  user_id: string | null
  username: string
  display_name: string
  avatar_emoji: string
}

/** TG-409: a quoted slice of the replied-to message; `offset` in UTF-16 code units. */
export interface ReplyQuote {
  text: string
  offset: number
}

export interface ForwardedFrom {
  sender: string
  /** Frozen spelling — pre-rename clients string-match it (CONTEXT.md "Room"). */
  room_name: string
}

/**
 * TG-304: one formatted range of a message's text. `offset`/`length` are UTF-16 code units
 * (JavaScript string indices). Type-specific fields are omitted unless the type uses them.
 * The wire omits `entities` when a message has none — read absent as `[]`.
 */
export interface MessageEntity {
  type: string
  offset: number
  length: number
  custom_emoji_id?: string
  url?: string
  user_id?: string
  language?: string
}

export type StickerFormat = 'webp' | 'tgs' | 'webm'

/**
 * TG-302/TG-303: the `sticker` field of a sticker message, present only with
 * `media_kind: 'sticker'` (both omitted on every other message). The file is the message's
 * own `attachment.download_url`; `set_short_name` opens the pack.
 */
export interface MessageSticker {
  sticker_id: string
  set_id: string
  set_short_name: string
  emoji: string
  format: StickerFormat
  width: number
  height: number
}

/**
 * TG-401: a voice message's playback projection. `waveform` is exactly 100 samples in 0..31,
 * evenly spread over `duration_ms`. `listened` is per viewer: for the sender, "someone else
 * played it"; for everyone else, "I played it". See docs/devlog/TG-401.md.
 */
export interface VoiceNote {
  duration_ms: number
  waveform: number[]
  listened: boolean
}

/**
 * TG-402: a round video message's playback projection. `thumbnail` is base64 of a small JPEG
 * (the recorder's first frame) or null. `listened` is per viewer, exactly like
 * `VoiceNote.listened`; a first view reaches the sender as `voice_listened`.
 * See docs/devlog/TG-402.md.
 */
export interface VideoNote {
  duration_ms: number
  thumbnail: string | null
  listened: boolean
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

/** TG-407: a location message's place. `live_until` is present on live locations. */
export interface MessageLocation {
  latitude: number
  longitude: number
  accuracy_m?: number
  heading?: number
  title?: string
  address?: string
  live_until?: string
  updated_at: string
}

/** TG-407: a location being shared live in a chat right now. */
export interface LiveLocationEntry {
  message_id: string
  sender_id: string | null
  sender: string
  location: MessageLocation
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
  /** TG-406: present exactly when the message carries a poll; omitted otherwise. */
  poll?: PollState
  /** TG-304: omitted when the message has no entities. */
  entities?: MessageEntity[]
  /** TG-302: `'sticker'` for a sticker message; omitted otherwise. */
  media_kind?: string
  /** TG-302: omitted unless `media_kind === 'sticker'` (and on recall). */
  sticker?: MessageSticker
  /** TG-204: the forum topic; omitted (or null) for General and non-forum chats. */
  topic_id?: string | null
  /** TG-202: a channel post's view count; omitted for every other message. */
  views?: number
  /** TG-202: a signed channel post's author; omitted when unsigned. */
  post_author?: string
  /** TG-203: a channel post's comment count; omitted unless the post has a comment thread. */
  comments?: number
  /** TG-401: present exactly when the message is a voice message; omitted otherwise. */
  voice?: VoiceNote
  /** TG-402: present exactly when the message is a round video message; omitted otherwise. */
  video_note?: VideoNote
  /** TG-404: true when sent without notifications; omitted otherwise. */
  silent?: boolean
  /** TG-410: a shared contact card; omitted otherwise. */
  contact?: ContactCard
  /** TG-407: present exactly when the message is a location. */
  location?: MessageLocation
  /** TG-403: shared by the 2–10 items of one album; omitted for every other message. */
  grouped_id?: string
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

/**
 * Mirrors of every WebSocket frame in the frozen TG-007 interface.
 *
 * The byte-level truth is pinned server-side by `tests/ws_frame_snapshot_legacy_test.rs`
 * and `tests/ws_frame_snapshot_extension_test.rs`; the shapes here transcribe those
 * fixtures. From TG-007 on the protocol only grows (new frames, new optional fields) —
 * additive edits only.
 *
 * Serde optionality notes transcribed from the snapshots:
 * - `broadcast`: every nullable field is PRESENT AS NULL when absent, except
 *   `client_message_id`, which is OMITTED when absent.
 * - `typing`: `user_id`/`username` are OMITTED when absent; `action` is optional on input
 *   (missing or unknown → `'typing'`) and always present on server output.
 * - `system`: `members`/`participants` are OMITTED when absent.
 * - `chat.membership_status`/`membership_role` (inside `chat_updated`): omitted when null.
 */
import type { Chat, ChatMember, ChatMembership } from './chat'
import type { Attachment, ForwardedFrom, MessageReaction, ReadReceipt, ReplyPreview } from './message'

/** The ten typing actions. Unknown wire strings MUST degrade to `'typing'` (TG-007 §1). */
export const TYPING_ACTIONS = [
  'typing',
  'recording_voice',
  'recording_video_note',
  'uploading_photo',
  'uploading_video',
  'uploading_document',
  'uploading_voice',
  'choosing_sticker',
  'choosing_location',
  'cancel',
] as const

export type TypingAction = (typeof TYPING_ACTIONS)[number]

/** Per-user status; the five obscured kinds are TG-505's privacy-tier placeholders. */
export type UserStatus =
  | { kind: 'online' }
  | { kind: 'offline'; last_seen: string }
  | { kind: 'recently' }
  | { kind: 'within_week' }
  | { kind: 'within_month' }
  | { kind: 'long_ago' }
  | { kind: 'empty' }

export interface UserStatusEntry {
  user_id: string
  status: UserStatus
}

/** The persisted-message broadcast frame (wire shape only; view fields live in domain/). */
export interface BroadcastFrame {
  type: 'broadcast'
  message_id: string
  client_message_id?: string | null
  sender_id: string | null
  sender: string
  sender_avatar: string
  content: string
  attachment: Attachment | null
  reply_to: ReplyPreview | null
  recalled_at: string | null
  edited_at: string | null
  timestamp: string
  favorite_id?: string | null
  forwarded_from: ForwardedFrom | null
  reactions: MessageReaction[]
}

export interface TypingFrame {
  type: 'typing'
  content: string
  action: TypingAction
  user_id?: string
  username?: string
}

export interface TopicSummary {
  id: string
  title: string
  icon_emoji: string
  closed: boolean
  pinned: boolean
}

export interface MessageViewCount {
  message_id: string
  views: number
}

export interface PollOption {
  text: string
  voters: number
}

export interface PollState {
  id: string
  question: string
  closed: boolean
  total_voters: number
  options: PollOption[]
}

/**
 * TG-008's cloud-draft sync frame, routed by the transport to the drafting account's own
 * connections only. composerStore consumes this shape; the REST client is TG-008's module.
 */
export interface DraftUpdatedFrame {
  type: 'draft_updated'
  user_id: string
  text: string
  reply_to_message_id: string | null
  topic_id: string | null
  updated_at: string
}

/** Every server→client frame. */
export type ServerFrame =
  | {
      type: 'auth_ok'
      /** Frozen spelling — pre-rename clients read it (CONTEXT.md "Room"). */
      room_name: string
      members: ChatMember[]
      participants: ChatMember[]
      read_receipts: ReadReceipt[]
      statuses: UserStatusEntry[]
    }
  | { type: 'auth_fail'; reason: string }
  | { type: 'history_complete' }
  | BroadcastFrame
  | { type: 'read_receipt'; user_id: string; username: string; message_id: string }
  | { type: 'message_recalled'; message_id: string; recalled_at: string }
  | { type: 'message_edited'; message_id: string; content: string; edited_at: string }
  | { type: 'reaction_changed'; message_id: string; emoji: string; user_id: string; active: boolean }
  | TypingFrame
  | { type: 'presence'; members: ChatMember[]; participants: ChatMember[] }
  | { type: 'system'; content: string; members?: ChatMember[]; participants?: ChatMember[] }
  | { type: 'user_status'; user_id: string; status: UserStatus }
  | { type: 'chat_updated'; chat: Chat }
  | { type: 'member_updated'; member: ChatMembership }
  | { type: 'topic_updated'; topic: TopicSummary }
  | { type: 'message_views_updated'; views: MessageViewCount[] }
  | { type: 'poll_updated'; message_id: string; poll: PollState }
  | DraftUpdatedFrame

export type ServerFrameType = ServerFrame['type']

/** Every client→server frame the current protocol accepts. */
export type ClientFrame =
  | { type: 'join'; token: string }
  | { type: 'auth'; token: string; password: string }
  | { type: 'message'; content: string; reply_to?: string; client_message_id?: string }
  | { type: 'edit'; message_id: string; content: string }
  | { type: 'read'; message_id: string }
  | { type: 'recall'; message_id: string }
  | { type: 'reaction'; message_id: string; emoji: string; active: boolean }
  | { type: 'poke'; target_user_id: string }
  | { type: 'typing'; content: string; action?: TypingAction }

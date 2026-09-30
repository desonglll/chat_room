/**
 * Mirrors of the canonical chat contract (`/api/chats/*`, TG-004 devlog §5).
 *
 * These types model the CANONICAL dialect only: the display title is `title`, never the
 * deprecated `name` (that spelling exists solely on the `/api/rooms/*` alias and inside
 * `/api/conversations`, both of which die in M6 with the frozen clients). The server never
 * serialises `password_hash` or `access_hash`, so they do not exist here at all.
 */

/** The four conversation shapes. `src/chats/chat_type.rs` — lowercase on the wire. */
export type ChatType = 'private' | 'group' | 'supergroup' | 'channel'

export type JoinPolicy = 'open' | 'approval'

export type MembershipStatus = 'pending' | 'invited' | 'active' | 'banned'

export type MembershipRole = 'owner' | 'admin' | 'member'

/**
 * The chat descriptor as `GET /api/chats/:id` serialises `src/chats/models.rs::Chat`.
 *
 * `membership_status` / `membership_role` are OMITTED (not null) when the requester has no
 * membership row — `skip_serializing_if = "Option::is_none"` on the Rust side — hence
 * optional-and-absent rather than nullable here.
 */
export interface Chat {
  id: string
  chat_type: ChatType
  title: string
  has_password: boolean
  creator_user_id: string | null
  join_policy: JoinPolicy
  avatar_emoji: string
  description: string
  /** Public `@handle`; only a supergroup or channel may hold one. */
  username: string | null
  is_forum: boolean
  /** A channel's discussion group. */
  linked_chat_id: string | null
  slow_mode_seconds: number
  auto_delete_seconds: number
  signatures_enabled: boolean
  history_visible_to_new_members: boolean
  /** Projection maintained by membership transactions; authoritative count stays in SQL. */
  member_count: number
  membership_status?: MembershipStatus
  membership_role?: MembershipRole
  unread_count: number
  created_at: string
}

/** One member row as WS `presence` / `auth_ok` carry it (TG-007 left it unchanged). */
export interface ChatMember {
  user_id: string
  username: string
  avatar_emoji: string
}

/** REST membership row (`/api/chats/:id/members`), also the WS `member_updated` payload. */
export interface ChatMembership {
  user_id: string
  username: string
  avatar_emoji: string
  nickname: string
  role: MembershipRole
  status: MembershipStatus
  requested_at: string
  joined_at: string | null
}

export interface CreateChatRequest {
  title: string
  password: string | null
  join_policy: JoinPolicy
  avatar_emoji?: string
  description?: string
}

export interface UpdateChatRequest {
  title?: string
  current_password?: string
  new_password?: string
  join_policy?: JoinPolicy
  avatar_emoji?: string
  description?: string
}

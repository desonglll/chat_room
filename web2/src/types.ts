export interface User {
  id: string
  username: string
  avatar_emoji: string
  display_name: string
  signature: string
  homepage: string
  created_at: string
}

export interface UserSummary {
  id: string
  username: string
  avatar_emoji: string
  display_name: string
}

export interface AuthSession {
  token: string
  user: User
  expires_at: string
}

export interface Room {
  id: string
  name: string
  has_password: boolean
  creator_user_id: string | null
  join_policy: 'open' | 'approval'
  avatar_emoji: string
  description: string
  membership_status?: 'active' | 'pending' | string
  membership_role?: 'owner' | 'admin' | 'member' | string
  unread_count: number
  created_at: string
}

export interface MessagePreview {
  message_id: string
  sender_id: string | null
  sender: string
  content: string
  attachment_file_name: string | null
  recalled: boolean
  created_at: string
}

export interface Conversation {
  room_id: string
  kind: 'direct' | 'group'
  title: string
  alias: string
  avatar_emoji: string
  description: string
  group: Room | null
  peer: UserSummary | null
  unread_count: number
  pending_join_requests: number
  last_message: MessagePreview | null
  last_activity_at: string
  created_at: string
}

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

export interface MessageReaction {
  emoji: string
  user_ids: string[]
}

export interface StoredMessage {
  id: string
  client_message_id: string | null
  room_id: string
  sender_id: string | null
  sender: string
  sender_avatar: string
  content: string
  attachment: Attachment | null
  reply_to: ReplyPreview | null
  recalled_at: string | null
  edited_at: string | null
  created_at: string
  forwarded_from: { sender: string; room_name: string } | null
  reactions: MessageReaction[]
}

export interface RoomMember {
  user_id: string
  username: string
  avatar_emoji: string
}

export interface RoomMembership extends RoomMember {
  nickname: string
  role: string
  status: string
  requested_at: string
  joined_at: string | null
}

export interface SocialUser extends UserSummary {
  signature: string
  relationship: 'none' | 'friend' | 'incoming' | 'outgoing' | 'blocked' | string
}

export interface FriendRequest {
  user: UserSummary
  direction: 'incoming' | 'outgoing'
  created_at: string
}

export interface PublicConfig {
  max_upload_bytes: number
  ai_enabled: boolean
}

export interface AiSuggestions {
  summary: string
  suggestions: string[]
}

export interface AdminOverview {
  generated_at: string
  database_backend: string
  attachment_backend: string
  online_users: number
  websocket_connections: number
  orphan_retention_hours: number
  deleted_room_retention_days: number
  chat_rooms_locked: boolean
  runtime: {
    uptime_seconds: number
    requests: number
    failures: number
    active_requests: number
    average_latency_ms: number
    max_latency_ms: number
  }
  totals: {
    users: number
    active_sessions: number
    active_rooms: number
    soft_deleted_rooms: number
    messages: number
    messages_24h: number
    attachments: number
    attachments_24h: number
    pending_uploads: number
  }
  storage: {
    logical_bytes: number
    physical_bytes: number
    orphaned_attachments: number
    orphaned_bytes: number
    missing_hashes: number
  }
  top_rooms: Array<{
    id: string
    name: string
    messages: number
    active_members: number
    last_message_at: string | null
  }>
}

export type RoomSocketEvent =
  | ({ type: 'broadcast'; message_id: string; timestamp: string } & Omit<
      StoredMessage,
      'id' | 'room_id' | 'created_at'
    >)
  | { type: 'auth_ok'; room_name: string; members: RoomMember[]; participants: RoomMember[] }
  | { type: 'auth_fail'; reason: string }
  | { type: 'history_complete' }
  | { type: 'message_edited'; message_id: string; content: string; edited_at: string }
  | { type: 'message_recalled'; message_id: string; recalled_at: string }
  | { type: 'reaction_changed'; message_id: string; emoji: string; user_id: string; active: boolean }
  | { type: 'presence'; members: RoomMember[]; participants: RoomMember[] }
  | { type: 'system'; content: string; members?: RoomMember[]; participants?: RoomMember[] }
  | { type: 'typing'; content: string; user_id?: string; username?: string }
  | { type: 'read_receipt'; user_id: string; username: string; message_id: string }

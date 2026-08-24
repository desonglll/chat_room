export interface Room {
  id: string
  name: string
  has_password: boolean
  creator_user_id: string | null
  join_policy: 'open' | 'approval'
  avatar_emoji: string
  description: string
  membership_status?: 'pending' | 'invited' | 'active'
  membership_role?: 'owner' | 'admin' | 'member'
  unread_count: number
  created_at: string
}

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

export type SocialRelationship = 'none' | 'incoming' | 'outgoing' | 'friend' | 'blocked'

export interface SocialUser extends UserSummary {
  signature: string
  relationship: SocialRelationship
  remark: string
}

export interface FriendRequest {
  user: UserSummary
  direction: 'incoming' | 'outgoing'
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

export interface ConversationSummary {
  room_id: string
  kind: 'group' | 'direct'
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

export interface UpdateProfilePayload {
  avatar_emoji?: string
  display_name?: string
  signature?: string
  homepage?: string
}

export interface AuthSession {
  token: string
  user: User
  expires_at: string
}

export interface PublicConfig {
  max_upload_bytes: number
  ai_enabled: boolean
}

export interface AiSuggestions {
  summary: string
  suggestions: string[]
}

export interface Attachment {
  id: string
  file_name: string
  mime_type: string
  size_bytes: number
  download_url: string
  is_sensitive: boolean
}

export interface FavoriteItem {
  id: string
  kind: 'message' | 'video' | 'manual'
  title: string
  content: string
  source_message_id: string | null
  source_sender: string
  source_room_name: string
  attachment: Attachment | null
  created_at: string
  updated_at: string
}

export interface FavoriteForwardResult {
  favorite_id: string
  target_room_id: string
  forwarded_message_id: string | null
  skipped_reason: string | null
}

export interface AttachmentUploadSession {
  id: string
  room_id: string
  uploader_id: string
  file_name: string
  mime_type: string
  declared_size_bytes: number
  received_bytes: number
  fingerprint: string
  status: 'in_progress' | 'completed' | 'aborted'
  created_at: string
  updated_at: string
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

export interface AccountMessageEvent {
  type: 'new_message'
  message_id: string
  room_id: string
  room_name: string
  conversation_kind: 'group' | 'direct'
  conversation_title: string
  sender_id: string | null
  sender: string
  content: string
  attachment_file_name: string | null
  timestamp: string
  is_mention: boolean
}

export interface ReplyPreview {
  message_id: string
  sender: string
  content: string
  attachment_file_name: string | null
  recalled: boolean
}

export interface RoomMember {
  user_id: string
  username: string
  avatar_emoji: string
}

export interface ReadReceipt {
  user_id: string
  username: string
  message_id: string
}

export interface ForwardedFrom {
  sender: string
  room_name: string
}

export interface MessageReaction {
  emoji: string
  user_ids: string[]
}

export interface ForwardResult {
  message_id: string
  target_room_id: string
  forwarded_message_id: string | null
  skipped_reason: string | null
}

export interface BroadcastMessage {
  type: 'broadcast'
  message_id: string
  sender_id: string | null
  sender: string
  sender_avatar: string
  content: string
  attachment: Attachment | null
  reply_to: ReplyPreview | null
  recalled_at: string | null
  edited_at: string | null
  timestamp: string
  forwarded_from: ForwardedFrom | null
  reactions: MessageReaction[]
  client_message_id?: string | null
  delivery_state?: DeliveryState
  motion?: MessageMotion
}

export interface StoredMessage {
  id: string
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
  forwarded_from: ForwardedFrom | null
  reactions: MessageReaction[]
}

export interface SystemMessage {
  type: 'system'
  key: string
  content: string
  motion?: MessageMotion
}

export type UploadPhase = 'queued' | 'hashing' | 'uploading' | 'deduplicating' | 'finalizing'
export type UploadTaskStatus = 'pending' | 'failed'

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
export type MessageMotion = 'none' | 'incoming' | 'outgoing' | 'system'
export type DeliveryState = 'sending' | 'sent' | 'failed'
export type ChatStatus = 'idle' | 'connecting' | 'online' | 'offline' | 'failed'
export type SendShortcut = 'enter' | 'shift-enter'
export type FocusShortcut = 'space' | 'slash' | 'none'
export type ThemePreference = 'light' | 'dark' | 'system'

export interface PrivacyLockShortcut {
  code: string
  primary: boolean
  alt: boolean
  shift: boolean
}

export interface ChatPreferences {
  sendShortcut: SendShortcut
  focusShortcut: FocusShortcut
  privacyLockShortcut: PrivacyLockShortcut
  autoDisguiseEnabled: boolean
  notificationsEnabled: boolean
  notificationDetails: boolean
  rememberRoomPasswords: boolean
  avatarEmoji: string
  theme: ThemePreference
}

export interface TypingDraft {
  user_id: string
  username: string
  content: string
}

export interface RoomMembership {
  user_id: string
  username: string
  avatar_emoji: string
  nickname: string
  role: 'owner' | 'admin' | 'member'
  status: 'pending' | 'invited' | 'active'
  requested_at: string
  joined_at: string | null
}

export interface RoomUpdateResult {
  room: Room
  passwordChanged: boolean
  password: string
}

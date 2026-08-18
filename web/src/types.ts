export interface Room {
  id: string
  name: string
  has_password: boolean
  creator_user_id: string | null
  join_policy: 'open' | 'approval'
  membership_status?: 'pending' | 'invited' | 'active'
  membership_role?: 'owner' | 'admin' | 'member'
  unread_count: number
  created_at: string
}

export interface User {
  id: string
  username: string
  avatar_emoji: string
  created_at: string
}

export interface AuthSession {
  token: string
  user: User
  expires_at: string
}

export interface PublicConfig {
  max_upload_bytes: number
}

export interface Attachment {
  id: string
  file_name: string
  mime_type: string
  size_bytes: number
  download_url: string
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
  sender_id: string | null
  sender: string
  content: string
  attachment_file_name: string | null
  timestamp: string
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
}

export interface SystemMessage {
  type: 'system'
  key: string
  content: string
}

export type DisplayMessage = BroadcastMessage | SystemMessage
export type ChatStatus = 'idle' | 'connecting' | 'online' | 'offline' | 'failed'
export type SendShortcut = 'enter' | 'shift-enter'
export type FocusShortcut = 'space' | 'slash' | 'none'

export interface ChatPreferences {
  sendShortcut: SendShortcut
  focusShortcut: FocusShortcut
  notificationsEnabled: boolean
  notificationDetails: boolean
  avatarEmoji: string
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

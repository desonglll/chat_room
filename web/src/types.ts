export interface Room {
  id: string
  name: string
  has_password: boolean
  created_at: string
}

export interface User {
  id: string
  username: string
  created_at: string
}

export interface AuthSession {
  token: string
  user: User
  expires_at: string
}

export interface Attachment {
  id: string
  file_name: string
  mime_type: string
  size_bytes: number
  download_url: string
}

export interface BroadcastMessage {
  type: 'broadcast'
  message_id: string
  sender_id: string | null
  sender: string
  content: string
  attachment: Attachment | null
  timestamp: string
}

export interface StoredMessage {
  id: string
  room_id: string
  sender_id: string | null
  sender: string
  content: string
  attachment: Attachment | null
  created_at: string
}

export interface SystemMessage {
  type: 'system'
  key: string
  content: string
}

export type DisplayMessage = BroadcastMessage | SystemMessage
export type ChatStatus = 'idle' | 'connecting' | 'online' | 'offline' | 'failed'

export interface RoomUpdateResult {
  room: Room
  passwordChanged: boolean
  password: string
}

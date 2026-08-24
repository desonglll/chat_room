export interface AdminRuntimeMetrics {
  uptime_seconds: number
  requests: number
  failures: number
  active_requests: number
  average_latency_ms: number
  max_latency_ms: number
}

export interface AdminTotals {
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

export interface AdminStorageMetrics {
  logical_bytes: number
  physical_bytes: number
  orphaned_attachments: number
  orphaned_bytes: number
  missing_hashes: number
}

export interface AdminTopRoom {
  id: string
  name: string
  messages: number
  active_members: number
  last_message_at: string | null
}

export interface AdminOverview {
  generated_at: string
  database_backend: 'sqlite' | 'postgres'
  attachment_backend: 'local' | 'oss'
  online_users: number
  websocket_connections: number
  orphan_retention_hours: number
  deleted_room_retention_days: number
  chat_rooms_locked: boolean
  runtime: AdminRuntimeMetrics
  totals: AdminTotals
  storage: AdminStorageMetrics
  top_rooms: AdminTopRoom[]
}

export interface AdminSystemLockStatus {
  locked: boolean
}

export interface AdminPurgeResult {
  attachment_objects_deleted: number
  attachment_bytes_deleted: number
  rooms_deleted: number
}

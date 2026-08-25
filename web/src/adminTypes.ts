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

export type AdminServiceState = 'healthy' | 'degraded' | 'disabled' | 'configured'

export interface AdminServiceStatus {
  id: 'database' | 'redis' | 'vector_store' | 'embedding' | 'ai_provider'
  label: string
  state: AdminServiceState
  latency_ms: number | null
  detail: string
}

export interface AdminVectorIndexStatus {
  points: number | null
  pending_jobs: number
  retrying_jobs: number
  last_error: string | null
}

export interface AdminServiceOverview {
  items: AdminServiceStatus[]
  vector_index: AdminVectorIndexStatus
}

export interface AdminVectorProbeMatch {
  message_id: string
  score: number
  sender: string
  content: string
  created_at: string
}

export interface AdminVectorProbeResult {
  latency_ms: number
  matches: AdminVectorProbeMatch[]
}

export interface AdminAiModelOption {
  id: string
  label: string
  provider: 'openai' | 'anthropic'
  base_url: string
  model: string
  api_key_env: string
  enabled: boolean
  ready: boolean
  source: 'environment' | 'database'
  created_at: string | null
  updated_at: string | null
}

export interface SaveAdminAiModelOption {
  label: string
  provider: 'openai' | 'anthropic'
  base_url: string
  model: string
  api_key_env: string
  enabled: boolean
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
  services: AdminServiceOverview
  top_rooms: AdminTopRoom[]
}

export interface AdminSystemLockStatus {
  locked: boolean
}

export interface AdminRoomLockStatus {
  room_id: string
  locked: boolean
}

export interface AdminPurgeResult {
  attachment_objects_deleted: number
  attachment_bytes_deleted: number
  rooms_deleted: number
}

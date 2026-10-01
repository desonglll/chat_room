/**
 * TG-705 system administration (`/api/admin/*`). Every call is refused with 403 for anyone
 * who is not a system administrator; `isAdmin` uses that to decide whether to show the console.
 * AI governance and model endpoints stay unused while AI features are switched off.
 */
import type { User } from '../types'
import { ApiError, encodePathSegment, type ApiClient } from './http'

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
  services: { items: { id: string; label: string; state: string; latency_ms: number | null; detail: string }[] }
  top_rooms: { id: string; name: string; messages: number; active_members: number; last_message_at: string | null }[]
}

export interface SystemAdmin {
  user: User
  granted_by: string | null
  grant_source: string
  created_at: string
}

export interface PurgeResult {
  attachment_objects_deleted: number
  attachment_bytes_deleted: number
  rooms_deleted: number
}

export interface AdminApi {
  /** True for a system administrator; false on 401/403. */
  isAdmin(): Promise<boolean>
  overview(): Promise<AdminOverview>
  setGlobalLock(locked: boolean): Promise<boolean>
  chatLock(chatId: string): Promise<boolean>
  setChatLock(chatId: string, locked: boolean): Promise<boolean>
  createInvite(lifetimeHours: number): Promise<{ token: string; expires_at: string }>
  admins(): Promise<SystemAdmin[]>
  grant(userId: string): Promise<SystemAdmin>
  revoke(userId: string): Promise<void>
  purge(): Promise<PurgeResult>
}

export function createAdminApi(client: ApiClient, token: () => string | null): AdminApi {
  const auth = () => {
    const value = token()
    return value ? { token: value } : {}
  }
  const chatLock = (chatId: string) => `/api/admin/room-locks/${encodePathSegment(chatId)}`
  return {
    isAdmin: () =>
      client.request('GET', '/api/admin/overview', auth()).then(
        () => true,
        (error: unknown) => {
          if (error instanceof ApiError && (error.status === 401 || error.status === 403)) return false
          throw error
        },
      ),
    overview: () => client.json<AdminOverview>('GET', '/api/admin/overview', auth()),
    setGlobalLock: async (locked) =>
      (await client.json<{ locked: boolean }>('PUT', '/api/admin/chat-lock', { ...auth(), body: { locked } })).locked,
    chatLock: async (chatId) => (await client.json<{ locked: boolean }>('GET', chatLock(chatId), auth())).locked,
    setChatLock: async (chatId, locked) =>
      (await client.json<{ locked: boolean }>('PUT', chatLock(chatId), { ...auth(), body: { locked } })).locked,
    createInvite: (lifetimeHours) =>
      client.json('POST', '/api/admin/registration-invites', { ...auth(), body: { lifetime_hours: lifetimeHours } }),
    admins: () => client.json<SystemAdmin[]>('GET', '/api/admin/system-admins', auth()),
    grant: (userId) => client.json<SystemAdmin>('PUT', `/api/admin/system-admins/${encodePathSegment(userId)}`, auth()),
    revoke: async (userId) => {
      await client.request('DELETE', `/api/admin/system-admins/${encodePathSegment(userId)}`, auth())
    },
    purge: () => client.json<PurgeResult>('POST', '/api/admin/maintenance/purge', auth()),
  }
}

/** `1536` → `1.5 KB` — sizes on the overview. */
export function formatStorageBytes(bytes: number): string {
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let value = bytes
  let unit = 0
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024
    unit += 1
  }
  return `${unit === 0 ? value : value.toFixed(1)} ${units[unit]}`
}

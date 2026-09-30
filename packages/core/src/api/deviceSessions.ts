/**
 * Active device sessions of the signed-in account (`src/accounts/sessions.rs`), consumed by
 * Settings → 设备 (TG-110). `id` is an opaque management id, never the bearer token.
 *
 * Revoking the current session is refused by the server (`409`, "use logout"), so the UI
 * never offers it; a session that vanished meanwhile (`404`) counts as revoked.
 */
import { encodePathSegment, type ApiClient } from './http'

export interface DeviceSession {
  id: string
  device_name: string
  ip_hint: string | null
  created_at: string
  last_used_at: string
  expires_at: string
  current: boolean
}

export function listDeviceSessions(client: ApiClient, token: string): Promise<DeviceSession[]> {
  return client.json<DeviceSession[]>('GET', '/api/users/me/sessions', { token })
}

export async function revokeDeviceSession(client: ApiClient, token: string, id: string): Promise<void> {
  await client.request('DELETE', `/api/users/me/sessions/${encodePathSegment(id)}`, {
    token,
    allowStatuses: [404],
  })
}

export async function revokeOtherDeviceSessions(client: ApiClient, token: string): Promise<void> {
  await client.request('DELETE', '/api/users/me/sessions/others', { token })
}

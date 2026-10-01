/** Mirrors of the account/user REST contract (`/api/users/*`, `/api/config`). */

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

export interface UpdateProfilePayload {
  avatar_emoji?: string
  display_name?: string
  signature?: string
  homepage?: string
}

/** `POST /api/users/register` / `POST /api/users/login` response. */
export interface AuthSession {
  token: string
  user: User
  expires_at: string
}

export type AiRuntimeStatus = 'disabled' | 'missing_credentials' | 'ready'

export type RegistrationMode = 'open' | 'invite_only' | 'disabled'

/** `GET /api/config` — the unauthenticated deployment descriptor. */
export interface PublicConfig {
  max_upload_bytes: number
  ai_enabled: boolean
  ai_status: AiRuntimeStatus
  registration_mode: RegistrationMode
  /** TG-407: map tile template (`{z}/{x}/{y}`) and its attribution; absent on older servers. */
  map_tile_url?: string
  map_attribution?: string
}

/**
 * Privacy settings HTTP client (TG-505).
 *
 * Wire contract frozen in `docs/devlog/TG-505.md`: `GET /api/users/me/privacy` answers every
 * dimension in `PRIVACY_KEYS` order; `PUT /api/users/me/privacy/:key` replaces one dimension's
 * tier and both exception lists and answers the stored rule. Account search for the exception
 * picker uses the existing `GET /api/users/search`.
 *
 * Phone number is deliberately absent: accounts in this product have no phone numbers.
 */
import { encodePathSegment, QueryParams, type ApiClient } from './http'

export const PRIVACY_KEYS = ['last_seen', 'profile_photo', 'forwards', 'group_invites', 'voice_messages'] as const
export type PrivacyKey = (typeof PRIVACY_KEYS)[number]

export const PRIVACY_TIERS = ['everybody', 'contacts', 'nobody'] as const
export type PrivacyTier = (typeof PRIVACY_TIERS)[number]

/** The account summary the server returns inside exception lists and search results. */
export interface PrivacyUser {
  id: string
  username: string
  avatar_emoji: string
  display_name: string
}

export interface PrivacyRule {
  key: PrivacyKey
  tier: PrivacyTier
  allow_users: PrivacyUser[]
  deny_users: PrivacyUser[]
}

export interface PrivacySettings {
  rules: PrivacyRule[]
}

export interface PrivacyRuleWrite {
  tier: PrivacyTier
  allow_user_ids: string[]
  deny_user_ids: string[]
}

/**
 * The PUT body for a rule. Like Telegram, a list the tier makes meaningless is dropped:
 * "always allow" under `everybody` and "never allow" under `nobody` change nothing.
 */
export function privacyRuleWrite(rule: PrivacyRule): PrivacyRuleWrite {
  return {
    tier: rule.tier,
    allow_user_ids: rule.tier === 'everybody' ? [] : rule.allow_users.map((user) => user.id),
    deny_user_ids: rule.tier === 'nobody' ? [] : rule.deny_users.map((user) => user.id),
  }
}

export interface PrivacyApi {
  get(): Promise<PrivacySettings>
  put(key: PrivacyKey, write: PrivacyRuleWrite): Promise<PrivacyRule>
  /** Username search for the exception picker (2–64 characters, server-enforced). */
  searchUsers(query: string): Promise<PrivacyUser[]>
}

export function createPrivacyApi(client: ApiClient, token: () => string | null): PrivacyApi {
  const auth = () => {
    const value = token()
    return value ? { token: value } : {}
  }
  return {
    get: () => client.json<PrivacySettings>('GET', '/api/users/me/privacy', auth()),
    put: (key, write) =>
      client.json<PrivacyRule>('PUT', `/api/users/me/privacy/${encodePathSegment(key)}`, { ...auth(), body: write }),
    searchUsers: (query) =>
      client.json<PrivacyUser[]>('GET', '/api/users/search', {
        ...auth(),
        query: new QueryParams({ q: query.trim(), limit: '20' }),
      }),
  }
}

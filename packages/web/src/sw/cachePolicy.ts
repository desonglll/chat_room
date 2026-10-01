/**
 * TG-601: what the service worker does with each request, and which cached entries to drop.
 * Pure so it is tested without a worker. Offline scope (the definition the task asks for):
 *
 * - the app shell and its hashed assets always load once they were fetched online;
 * - the chat list (`/api/conversations`, `/api/chats`) and every message page already opened
 *   online stay readable offline (network first, cache when the network fails);
 * - media already viewed (attachments, avatars, stickers) stays viewable, within the user's
 *   cache size and retention settings (TG-509);
 * - sending needs a connection; the composer keeps its draft locally and syncs it when back.
 * Everything else (writes, sockets, search, previews) goes to the network untouched.
 */

export type RequestKind = 'shell' | 'asset' | 'api-read' | 'media' | 'bypass'

export const CACHE_SHELL = 'tg-shell-v1'
export const CACHE_API = 'tg-api-v1'
export const CACHE_MEDIA = 'tg-media-v1'
export const ALL_CACHES = [CACHE_SHELL, CACHE_API, CACHE_MEDIA]

const API_READS = [
  /^\/api\/conversations$/,
  /^\/api\/chats$/,
  /^\/api\/chats\/[^/]+$/,
  /^\/api\/chats\/[^/]+\/messages$/,
]
const MEDIA = [/^\/api\/attachments\//, /^\/api\/users\/[^/]+\/avatar$/, /^\/api\/stickers\/[^/]+\/file/, /\/download$/]

/**
 * `ranged`: the request carries a `Range` header — how `<audio>`/`<video>` load voice, round
 * video and GIF media. Those go straight to the network: the answer is a 206 the Cache API
 * refuses to store, and a refused `put` fails the whole response (TG-1202).
 */
export function classify(method: string, url: URL, origin: string, navigate: boolean, ranged = false): RequestKind {
  if (method !== 'GET' || url.origin !== origin || ranged) return 'bypass'
  if (navigate) return 'shell'
  if (url.pathname.startsWith('/assets/')) return 'asset'
  if (API_READS.some((pattern) => pattern.test(url.pathname))) return 'api-read'
  if (MEDIA.some((pattern) => pattern.test(url.pathname))) return 'media'
  return 'bypass'
}

export interface CachedEntry {
  url: string
  bytes: number
  cachedAt: number
}

export interface CacheLimits {
  limitMb: number
  /** 0 = keep forever. */
  retentionDays: number
}

export const DEFAULT_LIMITS: CacheLimits = { limitMb: 1024, retentionDays: 7 }

/** Entries to delete: past retention, then the oldest until the rest fit in the size limit. */
export function entriesToEvict(entries: readonly CachedEntry[], limits: CacheLimits, now: number): string[] {
  const maxAge = limits.retentionDays > 0 ? limits.retentionDays * 86_400_000 : Infinity
  const expired = new Set(entries.filter((entry) => now - entry.cachedAt > maxAge).map((entry) => entry.url))
  const kept = entries.filter((entry) => !expired.has(entry.url)).sort((a, b) => b.cachedAt - a.cachedAt)
  const budget = Math.max(0, limits.limitMb) * 1024 * 1024
  let used = 0
  for (const entry of kept) {
    used += entry.bytes
    if (used > budget) expired.add(entry.url)
  }
  return [...expired]
}

export interface PushContent {
  title: string
  options: { body?: string; tag?: string; silent?: boolean; data: { url: string }; icon: string; badge: string }
}

/** The server's push payload (`src/push_notifications`) as a notification. */
export function pushContent(raw: unknown): PushContent {
  const payload = (raw && typeof raw === 'object' ? raw : {}) as Record<string, unknown>
  const url = typeof payload.url === 'string' && payload.url.startsWith('/') ? payload.url : '/'
  return {
    title: typeof payload.title === 'string' ? payload.title : 'Echo Gate',
    options: {
      ...(typeof payload.body === 'string' ? { body: payload.body } : {}),
      ...(typeof payload.tag === 'string' ? { tag: payload.tag } : {}),
      ...(payload.silent === true ? { silent: true } : {}),
      data: { url },
      icon: '/pwa-192.png',
      badge: '/pwa-192.png',
    },
  }
}

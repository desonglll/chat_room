/// <reference lib="webworker" />
/**
 * TG-601 service worker, built by Vite to `/sw.js` (vite.config.ts). Strategies per
 * `cachePolicy.classify`: shell network-first (offline → cached shell), hashed assets
 * cache-first, chat reads network-first with a cache fallback, media cache-first within the
 * user's limits. Push shows the notification; a click focuses or opens the app at its URL.
 */
import type { CachedEntry, CacheLimits } from './cachePolicy'
import {
  ALL_CACHES,
  CACHE_API,
  CACHE_MEDIA,
  CACHE_SHELL,
  classify,
  DEFAULT_LIMITS,
  entriesToEvict,
  pushContent,
} from './cachePolicy'

declare const self: ServiceWorkerGlobalScope

const CACHED_AT = 'x-tg-cached-at'
let limits: CacheLimits = DEFAULT_LIMITS

// No skipWaiting on install: a new version waits until the user accepts the update prompt
// (the client posts `skip-waiting`), so a running session is never swapped underneath.

self.addEventListener('activate', (event) => {
  event.waitUntil(
    (async () => {
      for (const key of await caches.keys()) if (!ALL_CACHES.includes(key)) await caches.delete(key)
      await self.clients.claim()
    })(),
  )
})

async function stamp(response: Response): Promise<Response> {
  const headers = new Headers(response.headers)
  headers.set(CACHED_AT, String(Date.now()))
  return new Response(await response.blob(), { status: response.status, statusText: response.statusText, headers })
}

async function networkFirst(request: Request, cacheName: string, fallback?: string): Promise<Response> {
  const cache = await caches.open(cacheName)
  try {
    const response = await fetch(request)
    if (response.ok) await cache.put(fallback ?? request, response.clone())
    return response
  } catch (error) {
    const cached = (await cache.match(fallback ?? request)) ?? (fallback ? undefined : await cache.match(request))
    if (cached) return cached
    throw error
  }
}

async function cacheFirst(request: Request, cacheName: string, trim: boolean): Promise<Response> {
  const cache = await caches.open(cacheName)
  const cached = await cache.match(request)
  if (cached) return cached
  const response = await fetch(request)
  // Only a complete 200 is cacheable; `cache.put` rejects a 206 and would fail the response.
  if (response.status === 200 && response.type === 'basic') {
    await cache.put(request, trim ? await stamp(response.clone()) : response.clone())
    if (trim) void trimMedia()
  }
  return response
}

async function trimMedia(): Promise<void> {
  const cache = await caches.open(CACHE_MEDIA)
  const entries: CachedEntry[] = []
  for (const request of await cache.keys()) {
    const response = await cache.match(request)
    if (!response) continue
    entries.push({
      url: request.url,
      bytes: Number(response.headers.get('content-length') ?? 0),
      cachedAt: Number(response.headers.get(CACHED_AT) ?? 0),
    })
  }
  for (const url of entriesToEvict(entries, limits, Date.now())) await cache.delete(url)
}

self.addEventListener('fetch', (event) => {
  const request = event.request
  const kind = classify(
    request.method,
    new URL(request.url),
    self.location.origin,
    request.mode === 'navigate',
    request.headers.has('range'),
  )
  if (kind === 'bypass') return
  if (kind === 'shell') event.respondWith(networkFirst(request, CACHE_SHELL, '/'))
  else if (kind === 'asset') event.respondWith(cacheFirst(request, CACHE_SHELL, false))
  else if (kind === 'api-read') event.respondWith(networkFirst(request, CACHE_API))
  else event.respondWith(cacheFirst(request, CACHE_MEDIA, true))
})

self.addEventListener('message', (event) => {
  const data = (event.data ?? {}) as { type?: string; limitMb?: number; retentionDays?: number }
  if (data.type === 'skip-waiting') void self.skipWaiting()
  if (data.type === 'cache-policy') {
    limits = {
      limitMb: Number(data.limitMb ?? limits.limitMb),
      retentionDays: Number(data.retentionDays ?? limits.retentionDays),
    }
    event.waitUntil(trimMedia())
  }
  // Signing out: nothing of this account may be readable offline afterwards.
  if (data.type === 'clear-user-data')
    event.waitUntil(Promise.all([caches.delete(CACHE_API), caches.delete(CACHE_MEDIA)]))
})

self.addEventListener('push', (event) => {
  let payload: unknown = {}
  try {
    payload = event.data?.json() ?? {}
  } catch {
    payload = {}
  }
  const { title, options } = pushContent(payload)
  event.waitUntil(self.registration.showNotification(title, options))
})

self.addEventListener('notificationclick', (event) => {
  event.notification.close()
  const url = (event.notification.data as { url?: string } | null)?.url ?? '/'
  event.waitUntil(
    (async () => {
      const windows = await self.clients.matchAll({ type: 'window', includeUncontrolled: true })
      const open = windows[0]
      if (open) {
        await open.focus()
        open.postMessage({ type: 'navigate', url })
        return
      }
      await self.clients.openWindow(url)
    })(),
  )
})

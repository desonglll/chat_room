import { createHash } from 'node:crypto'

export const PWA_CACHE_PREFIX = 'echo-gate-static-'

export const STATIC_PWA_ASSET_URLS = [
  '/manifest.webmanifest',
  '/favicon.svg',
  '/pwa-192.png',
  '/pwa-512.png',
  '/theme-bootstrap.js',
  '/brand/echo-gate.svg',
  '/icons/icon-sprite.svg',
  '/emoji-data-zh.json',
] as const

export interface PrecacheAsset {
  url: string
  content: string | Uint8Array
}

export function buildServiceWorker(assets: PrecacheAsset[]): string {
  const ordered = [...assets].sort((left, right) => left.url.localeCompare(right.url))
  const hash = createHash('sha256')
  for (const asset of ordered) {
    hash.update(asset.url)
    hash.update(asset.content)
  }
  const version = hash.digest('hex').slice(0, 16)
  const urls = [...new Set(ordered.map((asset) => asset.url))]

  return `const CACHE_PREFIX = ${JSON.stringify(PWA_CACHE_PREFIX)};
const CACHE_NAME = CACHE_PREFIX + ${JSON.stringify(version)};
const PRECACHE_URLS = ${JSON.stringify(urls)};
const PRECACHE_PATHS = new Set(PRECACHE_URLS);

self.addEventListener('install', (event) => {
  event.waitUntil(
    caches.open(CACHE_NAME)
      .then((cache) => cache.addAll(PRECACHE_URLS))
      .then(() => self.skipWaiting()),
  );
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches.keys()
      .then((names) => Promise.all(names.filter((name) => name.startsWith(CACHE_PREFIX) && name !== CACHE_NAME).map((name) => caches.delete(name))))
      .then(() => self.clients.claim()),
  );
});

self.addEventListener('fetch', (event) => {
  if (event.request.method !== 'GET') return;
  const url = new URL(event.request.url);
  if (url.origin !== self.location.origin || !PRECACHE_PATHS.has(url.pathname)) return;
  event.respondWith(caches.open(CACHE_NAME).then((cache) => cache.match(url.pathname).then((cached) => cached || fetch(event.request))));
});

self.addEventListener('message', (event) => {
  if (event.data?.type === 'SKIP_WAITING') self.skipWaiting();
  if (event.data?.type === 'CLEAR_STATIC_CACHES') {
    event.waitUntil(caches.keys().then((names) => Promise.all(names.filter((name) => name.startsWith(CACHE_PREFIX)).map((name) => caches.delete(name)))));
  }
});

self.addEventListener('push', (event) => {
  let payload = {};
  try { payload = event.data?.json() || {}; } catch { payload = {}; }
  const url = typeof payload.url === 'string' && payload.url.startsWith('/') && !payload.url.startsWith('//')
    ? payload.url
    : '/notifications';
  event.waitUntil(
    self.clients.matchAll({ type: 'window', includeUncontrolled: true }).then((windows) => {
      if (windows.some((client) => client.visibilityState === 'visible')) return;
      return self.registration.showNotification(
        typeof payload.title === 'string' ? payload.title : 'Echo Gate',
        {
          body: typeof payload.body === 'string' ? payload.body : '你有一条新通知',
          icon: '/pwa-192.png',
          badge: '/pwa-192.png',
          tag: typeof payload.tag === 'string' ? payload.tag : 'echo-gate-notification',
          data: { url },
        },
      );
    }),
  );
});

self.addEventListener('notificationclick', (event) => {
  event.notification.close();
  const candidate = event.notification.data?.url;
  const url = typeof candidate === 'string' && candidate.startsWith('/') && !candidate.startsWith('//')
    ? candidate
    : '/notifications';
  event.waitUntil(
    self.clients.matchAll({ type: 'window', includeUncontrolled: true }).then(async (windows) => {
      const client = windows[0];
      if (client) {
        await client.navigate(url);
        return client.focus();
      }
      return self.clients.openWindow(url);
    }),
  );
});
`
}

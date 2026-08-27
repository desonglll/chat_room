import { describe, expect, test } from 'bun:test'
import { buildServiceWorker, PWA_CACHE_PREFIX } from './pwaBuild'

describe('PWA service worker build', () => {
  test('pre-caches only the explicit static asset snapshot', () => {
    const worker = buildServiceWorker([
      { url: '/assets/app.abc.js', content: 'app' },
      { url: '/manifest.webmanifest', content: '{}' },
    ])

    expect(worker).toContain(PWA_CACHE_PREFIX)
    expect(worker).toContain('/assets/app.abc.js')
    expect(worker).toContain('/manifest.webmanifest')
    expect(worker).not.toContain('/api/')
    expect(worker).not.toContain('/ws')
    expect(worker).not.toContain('cache.put')
    expect(worker).toContain("self.addEventListener('push'")
    expect(worker).toContain("self.addEventListener('notificationclick'")
    expect(worker).toContain("'/notifications'")
  })

  test('changes the cache version when asset content changes', () => {
    const first = buildServiceWorker([{ url: '/assets/app.js', content: 'first' }])
    const second = buildServiceWorker([{ url: '/assets/app.js', content: 'second' }])
    expect(first).not.toBe(second)
  })
})

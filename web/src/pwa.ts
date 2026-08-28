import { onMounted, onUnmounted, ref } from 'vue'

export const PWA_CACHE_PREFIX = 'echo-gate-static-'

export function isPwaStaticCache(name: string): boolean {
  return name.startsWith(PWA_CACHE_PREFIX)
}

export function activateServiceWorker(worker: Pick<ServiceWorker, 'postMessage'> | null | undefined): boolean {
  if (!worker) return false
  worker.postMessage({ type: 'SKIP_WAITING' })
  return true
}

export async function clearPwaCaches(storage: Pick<CacheStorage, 'keys' | 'delete'> | undefined = globalThis.caches) {
  if (!storage) return
  const names = await storage.keys()
  await Promise.all(names.filter(isPwaStaticCache).map((name) => storage.delete(name)))
  navigator.serviceWorker?.controller?.postMessage({ type: 'CLEAR_STATIC_CACHES' })
}

export function usePwa() {
  const online = ref(navigator.onLine)
  const updateAvailable = ref(false)
  let registration: ServiceWorkerRegistration | null = null
  let reloading = false

  function syncOnline(): void {
    online.value = navigator.onLine
  }

  function watchInstalling(worker: ServiceWorker | null): void {
    if (!worker) return
    worker.addEventListener('statechange', () => {
      if (worker.state !== 'installed' || !navigator.serviceWorker.controller) return
      updateAvailable.value = true
      activateServiceWorker(worker)
    })
  }

  async function register(): Promise<void> {
    if (!import.meta.env.PROD || !('serviceWorker' in navigator)) return
    try {
      registration = await navigator.serviceWorker.register('/sw.js', { scope: '/', updateViaCache: 'none' })
      updateAvailable.value = Boolean(registration.waiting)
      activateServiceWorker(registration.waiting)
      registration.addEventListener('updatefound', () => watchInstalling(registration?.installing || null))
    } catch {
      // PWA support is optional; the connected web app remains fully usable.
    }
  }

  function applyUpdate(): void {
    activateServiceWorker(registration?.waiting)
  }

  function reloadForUpdate(): void {
    if (reloading) return
    reloading = true
    window.location.reload()
  }

  function checkForUpdate(): void {
    if (document.visibilityState === 'visible') void registration?.update()
  }

  onMounted(() => {
    window.addEventListener('online', syncOnline)
    window.addEventListener('offline', syncOnline)
    document.addEventListener('visibilitychange', checkForUpdate)
    navigator.serviceWorker?.addEventListener('controllerchange', reloadForUpdate)
    void register()
  })

  onUnmounted(() => {
    window.removeEventListener('online', syncOnline)
    window.removeEventListener('offline', syncOnline)
    document.removeEventListener('visibilitychange', checkForUpdate)
    navigator.serviceWorker?.removeEventListener('controllerchange', reloadForUpdate)
  })

  return { online, updateAvailable, applyUpdate }
}

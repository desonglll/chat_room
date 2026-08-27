import { onMounted, onUnmounted, ref } from 'vue'

export const PWA_CACHE_PREFIX = 'echo-gate-static-'

interface InstallPromptEvent extends Event {
  prompt(): Promise<void>
  userChoice: Promise<{ outcome: 'accepted' | 'dismissed'; platform: string }>
}

export function isPwaStaticCache(name: string): boolean {
  return name.startsWith(PWA_CACHE_PREFIX)
}

export async function clearPwaCaches(storage: Pick<CacheStorage, 'keys' | 'delete'> | undefined = globalThis.caches) {
  if (!storage) return
  const names = await storage.keys()
  await Promise.all(names.filter(isPwaStaticCache).map((name) => storage.delete(name)))
  navigator.serviceWorker?.controller?.postMessage({ type: 'CLEAR_STATIC_CACHES' })
}

export function usePwa() {
  const online = ref(navigator.onLine)
  const canInstall = ref(false)
  const updateAvailable = ref(false)
  let installPrompt: InstallPromptEvent | null = null
  let registration: ServiceWorkerRegistration | null = null
  let reloading = false

  function syncOnline(): void {
    online.value = navigator.onLine
  }

  function captureInstallPrompt(event: Event): void {
    event.preventDefault()
    installPrompt = event as InstallPromptEvent
    canInstall.value = true
  }

  function watchInstalling(worker: ServiceWorker | null): void {
    if (!worker) return
    worker.addEventListener('statechange', () => {
      if (worker.state === 'installed' && navigator.serviceWorker.controller) updateAvailable.value = true
    })
  }

  async function register(): Promise<void> {
    if (!import.meta.env.PROD || !('serviceWorker' in navigator)) return
    try {
      registration = await navigator.serviceWorker.register('/sw.js', { scope: '/', updateViaCache: 'none' })
      updateAvailable.value = Boolean(registration.waiting)
      registration.addEventListener('updatefound', () => watchInstalling(registration?.installing || null))
    } catch {
      // PWA support is optional; the connected web app remains fully usable.
    }
  }

  async function install(): Promise<void> {
    if (!installPrompt) return
    await installPrompt.prompt()
    await installPrompt.userChoice
    installPrompt = null
    canInstall.value = false
  }

  function applyUpdate(): void {
    registration?.waiting?.postMessage({ type: 'SKIP_WAITING' })
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
    window.addEventListener('beforeinstallprompt', captureInstallPrompt)
    document.addEventListener('visibilitychange', checkForUpdate)
    navigator.serviceWorker?.addEventListener('controllerchange', reloadForUpdate)
    void register()
  })

  onUnmounted(() => {
    window.removeEventListener('online', syncOnline)
    window.removeEventListener('offline', syncOnline)
    window.removeEventListener('beforeinstallprompt', captureInstallPrompt)
    document.removeEventListener('visibilitychange', checkForUpdate)
    navigator.serviceWorker?.removeEventListener('controllerchange', reloadForUpdate)
  })

  return { online, canInstall, updateAvailable, install, applyUpdate }
}

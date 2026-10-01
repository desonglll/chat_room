/**
 * TG-601: service-worker registration and the PWA state the UI shows — an update waiting to be
 * applied, and whether the browser offers installation. Production builds only (the dev server
 * has no `/sw.js`). The worker learns the cache limits from settings (TG-509) and is told to
 * forget the account's data on sign-out.
 */
import { createStore } from 'zustand/vanilla'
import type { SettingsStore } from '@tg/core'

interface InstallPromptEvent extends Event {
  prompt(): Promise<void>
  userChoice: Promise<{ outcome: 'accepted' | 'dismissed' }>
}

export const pwaStore = createStore<{ updateReady: boolean; canInstall: boolean }>()(() => ({
  updateReady: false,
  canInstall: false,
}))

let registration: ServiceWorkerRegistration | null = null
let installPrompt: InstallPromptEvent | null = null

const worker = () => (typeof navigator !== 'undefined' && 'serviceWorker' in navigator ? navigator.serviceWorker : null)

function watchUpdates(found: ServiceWorkerRegistration): void {
  if (found.waiting && worker()?.controller) pwaStore.setState({ updateReady: true })
  found.addEventListener('updatefound', () => {
    const installing = found.installing
    installing?.addEventListener('statechange', () => {
      // A worker waiting behind an existing controller is an update, not the first install.
      if (installing.state === 'installed' && worker()?.controller) pwaStore.setState({ updateReady: true })
    })
  })
}

export function registerPwa(settings: SettingsStore, navigate: (url: string) => void): void {
  const container = worker()
  if (!container || !import.meta.env.PROD) return
  const sendLimits = () => {
    const { cacheLimitMb, cacheRetentionDays } = settings.getState()
    container.controller?.postMessage({
      type: 'cache-policy',
      limitMb: cacheLimitMb,
      retentionDays: cacheRetentionDays,
    })
  }
  void container.register('/sw.js').then(
    (found) => {
      registration = found
      watchUpdates(found)
      sendLimits()
    },
    () => undefined,
  )
  settings.subscribe(sendLimits)
  // A notification click on an open window navigates in place (the worker posts the URL).
  container.addEventListener('message', (event: MessageEvent<{ type?: string; url?: string }>) => {
    if (event.data?.type === 'navigate' && event.data.url?.startsWith('/')) navigate(event.data.url)
  })
  window.addEventListener('beforeinstallprompt', (event) => {
    event.preventDefault()
    installPrompt = event as InstallPromptEvent
    pwaStore.setState({ canInstall: true })
  })
  window.addEventListener('appinstalled', () => {
    installPrompt = null
    pwaStore.setState({ canInstall: false })
  })
}

/** Activate the waiting version and reload into it (the user asked for it). */
export function applyUpdate(): void {
  const container = worker()
  if (!container || !registration?.waiting) return
  container.addEventListener('controllerchange', () => window.location.reload(), { once: true })
  registration.waiting.postMessage({ type: 'skip-waiting' })
}

export async function promptInstall(): Promise<void> {
  const prompt = installPrompt
  if (!prompt) return
  await prompt.prompt()
  await prompt.userChoice.catch(() => undefined)
  installPrompt = null
  pwaStore.setState({ canInstall: false })
}

/** Sign-out: drop every cached API response and media item of this account. */
export function clearOfflineData(): void {
  worker()?.controller?.postMessage({ type: 'clear-user-data' })
}

export function currentRegistration(): ServiceWorkerRegistration | null {
  return registration
}

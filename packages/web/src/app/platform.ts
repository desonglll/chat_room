/**
 * Browser implementations of `@tg/core`'s injected platform capabilities
 * (architecture.md §2). This file is the ONLY place the shell touches the
 * platform globals core is forbidden from naming; every feature imports these
 * adapters instead of reaching for `window` again.
 */
import type { CoreClock, CoreSocket, CoreSocketFactory, CoreStorage, FetchLike } from '@tg/core'

/**
 * `window.fetch` invoked ON `window` — an unbound `fetch` reference throws an
 * Illegal invocation in browsers, and core deliberately ships no global default
 * (a React Native host injects its own). Written as a call-time wrapper rather
 * than `.bind` at module scope so importing this file (e.g. from a component
 * under `bun test`, where no `window` exists) stays side-effect free.
 */
export const browserFetch: FetchLike = (path, init) => window.fetch(path, init)

/**
 * `localStorage` behind try/catch: in private windows or with storage disabled
 * every access can throw, and a shell that cannot persist a session should
 * still run — it just forgets on refresh.
 */
export const browserStorage: CoreStorage = {
  getItem(key) {
    try {
      return window.localStorage.getItem(key)
    } catch {
      return null
    }
  },
  setItem(key, value) {
    try {
      window.localStorage.setItem(key, value)
    } catch {
      // Storage unavailable — session simply does not survive a refresh.
    }
  },
  removeItem(key) {
    try {
      window.localStorage.removeItem(key)
    } catch {
      // Nothing to remove if nothing could be stored.
    }
  },
}

export const browserClock: CoreClock = {
  now: () => Date.now(),
  setTimeout: (callback, delayMs) => window.setTimeout(callback, delayMs),
  clearTimeout: (handle) => window.clearTimeout(handle as number),
}

/** WHATWG `WebSocket` → `CoreSocket`: handler properties, string frames only. */
export const createBrowserSocket: CoreSocketFactory = (url) => {
  const ws = new WebSocket(url)
  const socket: CoreSocket = {
    send: (data) => ws.send(data),
    close: (code, reason) => ws.close(code, reason),
    onopen: null,
    onmessage: null,
    onclose: null,
    onerror: null,
  }
  ws.onopen = () => socket.onopen?.()
  ws.onmessage = (event) => {
    if (typeof event.data === 'string') socket.onmessage?.(event.data)
  }
  ws.onclose = (event) => socket.onclose?.({ code: event.code, reason: event.reason })
  ws.onerror = () => socket.onerror?.()
  return socket
}

/** Same-origin `/ws/:room_id` URL (the frozen route parameter spelling). */
export function chatSocketUrl(chatId: string): string {
  const scheme = window.location.protocol === 'https:' ? 'wss' : 'ws'
  return `${scheme}://${window.location.host}/ws/${encodeURIComponent(chatId)}`
}

/** Same-origin `/ws/account` URL: cross-chat new-message events and unread snapshots. */
export function accountSocketUrl(): string {
  const scheme = window.location.protocol === 'https:' ? 'wss' : 'ws'
  return `${scheme}://${window.location.host}/ws/account`
}

/** Whether the page is on screen: a hidden tab must not advance read cursors. */
export function pageVisible(): boolean {
  return typeof document === 'undefined' || document.visibilityState === 'visible'
}

/** Subscribe to page visibility changes; returns the unsubscribe function. */
export function onPageVisible(handler: () => void): () => void {
  const listener = () => {
    if (pageVisible()) handler()
  }
  document.addEventListener('visibilitychange', listener)
  return () => document.removeEventListener('visibilitychange', listener)
}

/** Clipboard write; false when the browser refuses (insecure origin, permissions). */
export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text)
    return true
  } catch {
    return false
  }
}

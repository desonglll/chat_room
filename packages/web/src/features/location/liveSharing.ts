/**
 * TG-407: this browser's outgoing live locations. While one runs, the device position is
 * watched and sent at most every {@link MIN_INTERVAL_MS}; it stops by itself at `live_until`
 * (the server refuses later points anyway) or when the user stops it.
 */
import { createStore } from 'zustand/vanilla'
import type { LocationPointInput, LocationsApi } from '@tg/core'
import { approximatePoint } from '@tg/core'

export const MIN_INTERVAL_MS = 15_000

export interface LiveShare {
  chatId: string
  messageId: string
  until: number
  approximate: boolean
}

export const liveSharingStore = createStore<{ shares: LiveShare[] }>()(() => ({ shares: [] }))

const watchers = new Map<string, { watch: number; timer: ReturnType<typeof setTimeout> }>()

export function isSharing(messageId: string): boolean {
  return liveSharingStore.getState().shares.some((share) => share.messageId === messageId)
}

function forget(messageId: string): void {
  const watcher = watchers.get(messageId)
  if (watcher) {
    globalThis.navigator?.geolocation?.clearWatch(watcher.watch)
    clearTimeout(watcher.timer)
    watchers.delete(messageId)
  }
  liveSharingStore.setState((state) => ({ shares: state.shares.filter((share) => share.messageId !== messageId) }))
}

export function startSharing(share: LiveShare, api: LocationsApi): void {
  const geolocation = globalThis.navigator?.geolocation
  if (!geolocation || watchers.has(share.messageId)) return
  let lastSent = Date.now()
  const watch = geolocation.watchPosition(
    (position) => {
      const now = Date.now()
      if (now - lastSent < MIN_INTERVAL_MS || now >= share.until) return
      lastSent = now
      const point: LocationPointInput = {
        latitude: position.coords.latitude,
        longitude: position.coords.longitude,
        accuracy_m: position.coords.accuracy,
        ...(position.coords.heading !== null && Number.isFinite(position.coords.heading)
          ? { heading: Math.round(position.coords.heading) % 360 }
          : {}),
      }
      api.move(share.chatId, share.messageId, share.approximate ? approximatePoint(point) : point).catch(() =>
        // 409/404: sharing ended elsewhere (another tab, expiry) — stop watching.
        forget(share.messageId),
      )
    },
    () => undefined,
    { enableHighAccuracy: !share.approximate, maximumAge: 10_000 },
  )
  const timer = setTimeout(() => forget(share.messageId), Math.max(0, share.until - Date.now()))
  watchers.set(share.messageId, { watch, timer })
  liveSharingStore.setState((state) => ({ shares: [...state.shares, share] }))
}

export async function stopSharing(chatId: string, messageId: string, api: LocationsApi): Promise<void> {
  forget(messageId)
  await api.stop(chatId, messageId)
}

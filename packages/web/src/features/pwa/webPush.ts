/**
 * TG-601: this browser's Web Push subscription. The server holds the VAPID key
 * (`GET /api/push/config`) and the subscriptions (`/api/push/subscriptions`); a push click opens
 * the exact message (`/chat/:id?message=`).
 */
import { authStore, selectToken } from '@tg/core'
import { apiClient } from '../../app/client'
import { currentRegistration } from '../../app/pwa'

export type PushState = 'unsupported' | 'unavailable' | 'denied' | 'off' | 'on'

function base64UrlToBytes(value: string): Uint8Array {
  const padded = `${value}${'='.repeat((4 - (value.length % 4)) % 4)}`.replace(/-/g, '+').replace(/_/g, '/')
  const binary = atob(padded)
  return Uint8Array.from(binary, (character) => character.charCodeAt(0))
}

const auth = () => {
  const token = selectToken(authStore.getState())
  return token ? { token } : {}
}

async function vapidKey(): Promise<string | null> {
  const config = await apiClient.json<{ enabled: boolean; public_key: string | null }>('GET', '/api/push/config')
  return config.enabled ? config.public_key : null
}

export async function pushState(): Promise<PushState> {
  const registration = currentRegistration()
  if (!registration || typeof Notification === 'undefined' || !('pushManager' in registration)) return 'unsupported'
  if (Notification.permission === 'denied') return 'denied'
  if (!(await vapidKey().catch(() => null))) return 'unavailable'
  return (await registration.pushManager.getSubscription()) ? 'on' : 'off'
}

export async function enablePush(showDetails: boolean): Promise<PushState> {
  const registration = currentRegistration()
  const key = await vapidKey()
  if (!registration || !key) return 'unavailable'
  if ((await Notification.requestPermission()) !== 'granted') return 'denied'
  const subscription = await registration.pushManager.subscribe({
    userVisibleOnly: true,
    applicationServerKey: base64UrlToBytes(key) as BufferSource,
  })
  const json = subscription.toJSON()
  await apiClient.json('POST', '/api/push/subscriptions', {
    ...auth(),
    body: { endpoint: json.endpoint, keys: json.keys, show_details: showDetails },
  })
  return 'on'
}

export async function disablePush(): Promise<PushState> {
  const subscription = await currentRegistration()?.pushManager.getSubscription()
  if (subscription) {
    await apiClient
      .request('DELETE', '/api/push/subscriptions', { ...auth(), body: { endpoint: subscription.endpoint } })
      .catch(() => undefined)
    await subscription.unsubscribe()
  }
  return 'off'
}

export { base64UrlToBytes }

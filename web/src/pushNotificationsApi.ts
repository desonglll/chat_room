import { authHeaders, request } from './api'

interface WebPushPublicConfig {
  enabled: boolean
  public_key: string | null
}

interface PushSubscriptionShape {
  endpoint: string
  toJSON(): PushSubscriptionJSON
  unsubscribe(): Promise<boolean>
}

export function applicationServerKey(value: string): Uint8Array<ArrayBuffer> {
  const padding = '='.repeat((4 - (value.length % 4)) % 4)
  const decoded = atob((value + padding).replace(/-/g, '+').replace(/_/g, '/'))
  return Uint8Array.from(decoded, (character) => character.charCodeAt(0))
}

export function subscriptionBody(
  subscription: Pick<PushSubscriptionShape, 'endpoint' | 'toJSON'>,
  showDetails: boolean,
) {
  const keys = subscription.toJSON().keys
  if (!keys?.p256dh || !keys.auth) throw new Error('浏览器没有返回完整的 Push 密钥')
  return {
    endpoint: subscription.endpoint,
    keys: { p256dh: keys.p256dh, auth: keys.auth },
    show_details: showDetails,
  }
}

async function webPushConfig(): Promise<WebPushPublicConfig> {
  const response = await request('/api/push/config')
  if (!response.ok) throw new Error(`读取 Push 配置失败：${response.status}`)
  return response.json() as Promise<WebPushPublicConfig>
}

async function saveSubscription(token: string, subscription: PushSubscriptionShape, showDetails: boolean) {
  const response = await request('/api/push/subscriptions', {
    method: 'POST',
    headers: { ...authHeaders(token), 'Content-Type': 'application/json' },
    body: JSON.stringify(subscriptionBody(subscription, showDetails)),
  })
  if (response.status === 401) throw new Error('登录已过期')
  if (response.status === 409) throw new Error('此浏览器的 Push 订阅正在被另一个账户使用')
  if (!response.ok) throw new Error(`保存 Push 订阅失败：${response.status}`)
}

async function deleteSubscription(token: string, endpoint: string): Promise<void> {
  const response = await request('/api/push/subscriptions', {
    method: 'DELETE',
    headers: { ...authHeaders(token), 'Content-Type': 'application/json' },
    body: JSON.stringify({ endpoint }),
  })
  if (!response.ok && response.status !== 401) throw new Error(`撤销 Push 订阅失败：${response.status}`)
}

async function currentSubscription(): Promise<PushSubscriptionShape | null> {
  if (!('serviceWorker' in navigator)) return null
  const registration = await navigator.serviceWorker.getRegistration('/')
  if (!registration) return null
  return registration.pushManager.getSubscription()
}

export async function enableWebPush(token: string, showDetails: boolean): Promise<void> {
  const config = await webPushConfig()
  // Local foreground notifications remain available on deployments that do not configure VAPID.
  if (!config.enabled || !config.public_key) return
  if (!('serviceWorker' in navigator) || !('PushManager' in window)) {
    throw new Error('当前浏览器不支持后台消息通知')
  }
  const registration = await navigator.serviceWorker.ready
  const existing = await registration.pushManager.getSubscription()
  const subscription =
    existing ||
    (await registration.pushManager.subscribe({
      userVisibleOnly: true,
      applicationServerKey: applicationServerKey(config.public_key),
    }))
  await saveSubscription(token, subscription, showDetails)
}

export async function removeWebPushSubscription(token = ''): Promise<void> {
  const subscription = await currentSubscription()
  if (!subscription) return
  try {
    if (token) await deleteSubscription(token, subscription.endpoint)
  } finally {
    await subscription.unsubscribe()
  }
}

export async function syncWebPushSubscription(token: string, enabled: boolean, showDetails: boolean): Promise<void> {
  if (enabled) await enableWebPush(token, showDetails)
  else await removeWebPushSubscription(token)
}

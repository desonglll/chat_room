import { describe, expect, test } from 'bun:test'
import { applicationServerKey, subscriptionBody } from './pushNotificationsApi'

describe('Web Push browser contract', () => {
  test('decodes URL-safe unpadded VAPID public keys', () => {
    expect([...applicationServerKey('AQID-_8')]).toEqual([1, 2, 3, 251, 255])
  })

  test('sends only the endpoint, browser keys, and explicit detail preference', () => {
    const body = subscriptionBody(
      {
        endpoint: 'https://push.example/device-capability',
        toJSON: () => ({ endpoint: 'ignored', keys: { p256dh: 'public', auth: 'secret' } }),
      },
      false,
    )
    expect(body).toEqual({
      endpoint: 'https://push.example/device-capability',
      keys: { p256dh: 'public', auth: 'secret' },
      show_details: false,
    })
    expect(JSON.stringify(body)).not.toContain('message')
  })
})

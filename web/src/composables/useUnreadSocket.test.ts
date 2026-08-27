import { describe, expect, mock, test } from 'bun:test'
import { NOTIFICATIONS_CHANGED_EVENT, type NotificationsChangedSignal } from '../notificationsApi'
import { publishNotificationSignal } from './useUnreadSocket'

describe('account notification invalidation', () => {
  test('updates the badge callback and publishes a page refresh event', () => {
    const target = new EventTarget()
    const onChanged = mock(() => {})
    const received: NotificationsChangedSignal[] = []
    target.addEventListener(NOTIFICATIONS_CHANGED_EVENT, (event) => {
      received.push((event as CustomEvent<NotificationsChangedSignal>).detail)
    })
    const signal: NotificationsChangedSignal = {
      type: 'notifications_changed',
      unread_count: 4,
      latest_notification_id: 'notification-4',
    }

    publishNotificationSignal(signal, onChanged, target)

    expect(onChanged).toHaveBeenCalledWith(signal)
    expect(received).toEqual([signal])
  })
})

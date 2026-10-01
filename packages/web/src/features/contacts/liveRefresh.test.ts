// TG-801: the contacts page never reloaded on its own — a request arriving or being accepted
// only showed up after a manual refresh. `social_changed` now bumps a revision it watches.
import { expect, test } from 'bun:test'
import { applyAccountSignal, notificationsStore } from '../notifications/notificationsStore'

test('every social_changed frame bumps the revision the contacts page reloads on', () => {
  const before = notificationsStore.getState().socialRevision
  applyAccountSignal({ type: 'social_changed', incoming_request_count: 1 })
  applyAccountSignal({ type: 'social_changed', incoming_request_count: 1 })
  expect(notificationsStore.getState().socialRevision).toBe(before + 2)
  expect(notificationsStore.getState().incomingRequests).toBe(1)
})

test('other frames leave it alone', () => {
  const before = notificationsStore.getState().socialRevision
  applyAccountSignal({ type: 'notifications_changed', unread_count: 3 })
  expect(notificationsStore.getState().socialRevision).toBe(before)
})

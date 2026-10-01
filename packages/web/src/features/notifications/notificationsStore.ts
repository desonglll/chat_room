/**
 * TG-703: the live counters the account socket pushes — unread notifications
 * (`notifications_changed`) and incoming friend requests (`social_changed`).
 */
import { createStore } from 'zustand/vanilla'
import { authStore, createNotificationsApi, selectToken } from '@tg/core'
import { apiClient } from '../../app/client'

export const notificationsApi = createNotificationsApi(apiClient, () => selectToken(authStore.getState()) || null)

export const notificationsStore = createStore<{ unread: number; incomingRequests: number }>()(() => ({
  unread: 0,
  incomingRequests: 0,
}))

/** Feed one account-socket frame the chat list does not handle itself. */
export function applyAccountSignal(frame: { type: string } & Record<string, unknown>): void {
  if (frame.type === 'notifications_changed' && typeof frame.unread_count === 'number') {
    notificationsStore.setState({ unread: frame.unread_count })
  } else if (frame.type === 'social_changed' && typeof frame.incoming_request_count === 'number') {
    notificationsStore.setState({ incomingRequests: frame.incoming_request_count })
  }
}

export function refreshUnread(): void {
  notificationsApi.unreadCount().then(
    (unread) => notificationsStore.setState({ unread }),
    () => undefined,
  )
}

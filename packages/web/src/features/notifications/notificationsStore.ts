/**
 * TG-703: the live counters the account socket pushes — unread notifications
 * (`notifications_changed`) and incoming friend requests (`social_changed`).
 */
import { createStore } from 'zustand/vanilla'
import type { UserStatusEntry } from '@tg/core'
import { authStore, createNotificationsApi, selectToken } from '@tg/core'
import { apiClient } from '../../app/client'

export const notificationsApi = createNotificationsApi(apiClient, () => selectToken(authStore.getState()) || null)

export const notificationsStore = createStore<{
  unread: number
  incomingRequests: number
  /** TG-801: bumps on every `social_changed` frame so open contact views reload. */
  socialRevision: number
  /** TG-1102: friends' presence as last pushed (`friend_statuses`); null until the first push. */
  friendStatuses: UserStatusEntry[] | null
}>()(() => ({
  unread: 0,
  incomingRequests: 0,
  socialRevision: 0,
  friendStatuses: null,
}))

/** Feed one account-socket frame the chat list does not handle itself. */
export function applyAccountSignal(frame: { type: string } & Record<string, unknown>): void {
  if (frame.type === 'notifications_changed' && typeof frame.unread_count === 'number') {
    notificationsStore.setState({ unread: frame.unread_count })
  } else if (frame.type === 'social_changed' && typeof frame.incoming_request_count === 'number') {
    notificationsStore.setState((state) => ({
      incomingRequests: frame.incoming_request_count as number,
      socialRevision: state.socialRevision + 1,
    }))
  } else if (frame.type === 'friend_statuses' && Array.isArray(frame.statuses)) {
    notificationsStore.setState({ friendStatuses: frame.statuses as UserStatusEntry[] })
  }
}

export function refreshUnread(): void {
  notificationsApi.unreadCount().then(
    (unread) => notificationsStore.setState({ unread }),
    () => undefined,
  )
}

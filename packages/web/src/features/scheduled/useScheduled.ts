/**
 * TG-404: the app-wide scheduled actions and the per-chat hook the composer uses to know
 * whether to show the calendar entry. Delivered entries leave the list as soon as their
 * `broadcast` (same id) lands in the chat timeline.
 */
import { useEffect } from 'react'
import { useStore } from 'zustand/react'
import type { ScheduledMessage } from '@tg/core'
import { authStore, messageStore, selectToken } from '@tg/core'
import { apiClient } from '../../app/client'
import { createScheduledActions } from './scheduledActions'
import { scheduledStore } from './scheduledStore'

export const scheduledActions = createScheduledActions({
  client: apiClient,
  token: () => selectToken(authStore.getState()),
  store: scheduledStore,
})

const EMPTY: readonly ScheduledMessage[] = []

function deliveredIds(chatId: string): Set<string> {
  const ids = new Set<string>()
  for (const message of messageStore.getState().timelines[chatId]?.messages ?? []) {
    if (message.type === 'broadcast') ids.add(message.message_id)
  }
  return ids
}

export function useScheduledMessages(chatId: string): readonly ScheduledMessage[] {
  const token = useStore(authStore, selectToken)
  const items = useStore(scheduledStore, (state) => state.byChat[chatId]) ?? EMPTY

  useEffect(() => {
    if (!token) return
    scheduledActions.load(chatId).catch(() => undefined)
  }, [chatId, token])

  useEffect(() => {
    const sweep = () => {
      if ((scheduledStore.getState().byChat[chatId] ?? []).length === 0) return
      scheduledStore.getState().dropDelivered(chatId, deliveredIds(chatId))
    }
    sweep()
    return messageStore.subscribe(sweep)
  }, [chatId])

  return items
}

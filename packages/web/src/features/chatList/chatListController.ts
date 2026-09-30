/**
 * Chat-list data flow: REST → `chatListStore`. Framework-free and injected so the
 * store wiring is testable with a fake fetch (`chatListController.test.ts`).
 */
import type { ApiClient, Chat, ChatListStore } from '@tg/core'
import { ApiError, createChat, listChats } from '@tg/core'

export interface ChatListDeps {
  client: ApiClient
  token: string
  store: ChatListStore
}

/** Load the caller's chats into the store; errors land in `store.error` as a status. */
export async function loadChats({ client, token, store }: ChatListDeps): Promise<void> {
  store.getState().setLoading(true)
  try {
    store.getState().setChats(await listChats(client, token))
  } catch (error) {
    store.getState().setError(error instanceof ApiError ? `${error.status}` : 'network')
  } finally {
    store.getState().setLoading(false)
  }
}

/** Create an open, passwordless chat (Decision: the password UI is M1/M2 surface). */
export async function createNewChat({ client, token, store }: ChatListDeps, title: string): Promise<Chat> {
  const chat = await createChat(client, token, {
    title: title.trim(),
    password: null,
    join_policy: 'open',
  })
  store.getState().upsertChat(chat)
  return chat
}

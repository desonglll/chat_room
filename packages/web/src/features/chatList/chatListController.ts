/**
 * Chat-list data flow: REST → `chatListStore`. Framework-free and injected so the
 * store wiring is testable with a fake fetch (`chatListController.test.ts`).
 *
 * Two reads feed the list: `/api/chats` (canonical descriptors for the chat header and
 * info pane) and `/api/conversations` (the sidebar rows: preview, pin/archive/mute).
 */
import type { ApiClient, Chat, ChatListStore, ConversationSummary } from '@tg/core'
import { ApiError, createChat, listChats } from '@tg/core'

export interface ChatListDeps {
  client: ApiClient
  token: string
  store: ChatListStore
}

const failure = (error: unknown): string => (error instanceof ApiError ? `${error.status}` : 'network')

/** `GET /api/conversations` — read-only use of the route TG-208 is unifying. */
export function listConversations(client: ApiClient, token: string): Promise<ConversationSummary[]> {
  return client.json<ConversationSummary[]>('GET', '/api/conversations', { token })
}

/** Load the caller's chats into the store; errors land in `store.error` as a status. */
export async function loadChats({ client, token, store }: ChatListDeps): Promise<void> {
  store.getState().setLoading(true)
  try {
    store.getState().setChats(await listChats(client, token))
  } catch (error) {
    store.getState().setError(failure(error))
  } finally {
    store.getState().setLoading(false)
  }
}

/** Load the sidebar rows; a failure keeps the rows already shown and reports a status. */
export async function loadConversations({ client, token, store }: ChatListDeps): Promise<void> {
  store.getState().setLoading(true)
  try {
    store.getState().setConversations(await listConversations(client, token))
  } catch (error) {
    store.getState().setError(failure(error))
  } finally {
    store.getState().setLoading(false)
  }
}

/** Both reads, in parallel — the workspace's initial load and every resync. */
export async function loadChatList(deps: ChatListDeps): Promise<void> {
  await Promise.all([loadChats(deps), loadConversations(deps)])
}

/** Create an open, passwordless chat (Decision: the password UI is M1/M2 surface). */
export async function createNewChat({ client, token, store }: ChatListDeps, title: string): Promise<Chat> {
  const chat = await createChat(client, token, {
    title: title.trim(),
    password: null,
    join_policy: 'open',
  })
  store.getState().upsertChat(chat)
  // The new chat's sidebar row (preferences, preview) only exists server-side.
  void loadConversations({ client, token, store })
  return chat
}

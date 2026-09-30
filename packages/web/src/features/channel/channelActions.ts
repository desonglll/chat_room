/**
 * The subscriber bar's actions, framework-free with injected dependencies: subscribe, leave,
 * and the mute switch (the conversation preference every chat has, `chat_members.muted_until`
 * server-side — a subscriber's mute needs no channel-specific storage).
 */
import type { ApiClient, ChannelApi, ChatListStore, ConversationPreferences } from '@tg/core'
import { getChat } from '@tg/core'
import { notificationsPatch, updateConversationPreferences } from '../chatInfo/chatInfoApi'
import { loadConversations } from '../chatList/chatListController'

export interface ChannelActionDeps {
  api: ChannelApi
  client: ApiClient
  token: string
  store: ChatListStore
}

/** Subscribe, then refresh the descriptor (membership, subscriber count) and the sidebar. */
export async function subscribeToChannel(deps: ChannelActionDeps, chatId: string): Promise<'active' | 'pending'> {
  const { state } = await deps.api.subscribe(chatId)
  const chat = await getChat(deps.client, chatId, deps.token)
  if (chat) deps.store.getState().upsertChat(chat)
  void loadConversations(deps)
  return state
}

/** Unsubscribe and drop the channel from the sidebar. */
export async function leaveChannel(deps: ChannelActionDeps, chatId: string): Promise<void> {
  await deps.api.unsubscribe(chatId)
  const store = deps.store.getState()
  store.removeChat(chatId)
  store.setConversations(store.conversations.filter((row) => row.room_id !== chatId))
}

/** Mute or unmute, optimistic in the sidebar row, rolled back if the server refuses. */
export async function setChannelMuted(deps: ChannelActionDeps, chatId: string, muted: boolean): Promise<void> {
  const patch = notificationsPatch(!muted)
  const store = deps.store
  const before = store.getState().conversations.find((row) => row.room_id === chatId)?.preferences
  const apply = (preferences: ConversationPreferences) =>
    store
      .getState()
      .setConversations(
        store.getState().conversations.map((row) => (row.room_id === chatId ? { ...row, preferences } : row)),
      )
  if (before) apply({ ...before, ...patch })
  try {
    apply(await updateConversationPreferences(deps.client, deps.token, chatId, patch))
  } catch (error) {
    if (before) apply(before)
    throw error
  }
}

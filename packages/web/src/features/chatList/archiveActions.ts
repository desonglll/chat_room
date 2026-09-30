/**
 * Archive / unarchive a chat: optimistic in `chatListStore`, persisted with
 * `PATCH /api/conversations/{id}/preferences`, rolled back if the server refuses. Also the
 * live half of the unarchive-on-new-message rule for the account socket.
 *
 * Framework-free with injected deps, like `chatListController.ts`.
 */
import type { AccountMessageEvent, ApiClient, ChatListStore, ConversationPreferences } from '@tg/core'
import { unarchivesOnMessage, withPreferences } from './archiveRules'

export interface ArchiveDeps {
  client: ApiClient
  token: string
  store: ChatListStore
}

export function updateConversationPreferences(
  client: ApiClient,
  token: string,
  chatId: string,
  body: Partial<Pick<ConversationPreferences, 'is_archived' | 'is_pinned'>>,
): Promise<ConversationPreferences> {
  return client.json<ConversationPreferences>('PATCH', `/api/conversations/${encodeURIComponent(chatId)}/preferences`, {
    token,
    body,
  })
}

/** Resolves true when the server accepted the change; on failure the row returns to where it was. */
export async function setChatArchived(deps: ArchiveDeps, chatId: string, archived: boolean): Promise<boolean> {
  const { store } = deps
  const before = store.getState().conversations.find((conversation) => conversation.room_id === chatId)
  if (!before || before.preferences.is_archived === archived) return before !== undefined
  store.getState().setConversations(withPreferences(store.getState().conversations, chatId, { is_archived: archived }))
  try {
    const saved = await updateConversationPreferences(deps.client, deps.token, chatId, { is_archived: archived })
    store.getState().setConversations(withPreferences(store.getState().conversations, chatId, saved))
    return true
  } catch {
    store
      .getState()
      .setConversations(
        withPreferences(store.getState().conversations, chatId, { is_archived: before.preferences.is_archived }),
      )
    return false
  }
}

/**
 * The account socket delivered another member's message: mirror the server trigger so a
 * non-muted archived chat pops back into the main list at once.
 */
export function applyUnarchiveRule(
  store: ChatListStore,
  event: AccountMessageEvent,
  currentUserId: string,
  now: number,
) {
  const conversation = store.getState().conversations.find((candidate) => candidate.room_id === event.room_id)
  if (!conversation || !unarchivesOnMessage(conversation, event.sender_id, currentUserId, now)) return
  store
    .getState()
    .setConversations(withPreferences(store.getState().conversations, event.room_id, { is_archived: false }))
}

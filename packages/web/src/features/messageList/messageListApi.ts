/**
 * Binds the controller's `MessageListApi` to the REST history endpoints of one chat:
 * `GET /api/chats/:id/messages?before=` (older pages) and
 * `GET /api/chats/:id/messages/:message_id/context` (jump windows and newer pages).
 * Both authorize the reader server-side on every call.
 */
import type { ApiClient, BroadcastMessage, StoredMessage } from '@tg/core'
import { listChatMessageContext, listChatMessages, storedMessageToBroadcast } from '@tg/core'
import type { MessageListApi } from './messageListController'

const toBroadcasts = (page: StoredMessage[]): BroadcastMessage[] => page.map(storedMessageToBroadcast)

export function createMessageListApi(client: ApiClient, chatId: string, token: () => string): MessageListApi {
  return {
    loadOlder: async (beforeId, limit) =>
      toBroadcasts(await listChatMessages(client, chatId, { token: token() }, beforeId, limit)),
    loadAround: async (messageId, limit) =>
      toBroadcasts(await listChatMessageContext(client, chatId, messageId, { token: token() }, limit)),
  }
}

/**
 * `POST /api/chats/:id/pins/:message_id` — the server checks the `message.pin` permission
 * (or active membership in a private chat) and re-authorizes the message's chat. Kept
 * beside its only caller: `@tg/core/api` has no pins module yet and is not this task's.
 */
import type { ApiClient } from '@tg/core'
import { encodePathSegment } from '@tg/core'

export async function pinMessage(client: ApiClient, token: string, chatId: string, messageId: string): Promise<void> {
  await client.request('POST', `/api/chats/${encodePathSegment(chatId)}/pins/${encodePathSegment(messageId)}`, {
    token,
  })
}

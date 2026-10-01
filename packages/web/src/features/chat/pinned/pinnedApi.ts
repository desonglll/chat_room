/**
 * TG-901: the chat's pinned messages (`src/messages/pins.rs`). Reads need membership; unpin
 * needs the `message.pin` permission (any member in a private chat) — both checked server-side.
 */
import type { ApiClient, StoredMessage } from '@tg/core'
import { encodePathSegment } from '@tg/core'

export interface ChatPin {
  message: StoredMessage
  pinned_by: string
  pinned_at: string
}

const pinsPath = (chatId: string) => `/api/chats/${encodePathSegment(chatId)}/pins`

export function listPins(client: ApiClient, token: string, chatId: string): Promise<ChatPin[]> {
  return client.json<ChatPin[]>('GET', pinsPath(chatId), { token })
}

export async function unpinMessage(client: ApiClient, token: string, chatId: string, messageId: string): Promise<void> {
  await client.request('DELETE', `${pinsPath(chatId)}/${encodePathSegment(messageId)}`, { token })
}

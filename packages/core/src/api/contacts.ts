/**
 * TG-410: contact cards and translation. Translation is a projection for one viewer through
 * the server's AI provider; `translationAvailable()` is false when none is configured, and the
 * client then hides the entry rather than showing an error.
 */
import type { StoredMessage } from '../types'
import type { ApiClient } from './http'
import { encodePathSegment } from './http'

export interface ContactsApi {
  sendContact(
    chatId: string,
    userId: string,
    options?: { replyTo?: string | null; topicId?: string | null },
  ): Promise<StoredMessage>
  translationAvailable(): Promise<boolean>
  translate(messageId: string, targetLanguage: string): Promise<string>
}

export function createContactsApi(client: ApiClient, token: () => string | null): ContactsApi {
  const auth = () => {
    const value = token()
    return value ? { token: value } : {}
  }
  return {
    sendContact: (chatId, userId, options = {}) =>
      client.json<StoredMessage>('POST', `/api/chats/${encodePathSegment(chatId)}/contact-messages`, {
        ...auth(),
        body: {
          user_id: userId,
          ...(options.replyTo ? { reply_to: options.replyTo } : {}),
          ...(options.topicId ? { topic_id: options.topicId } : {}),
        },
      }),
    translationAvailable: () =>
      client
        .json<{ available: boolean }>('GET', '/api/translation', auth())
        .then((result) => result.available)
        .catch(() => false),
    translate: (messageId, targetLanguage) =>
      client
        .json<{ text: string }>('POST', `/api/messages/${encodePathSegment(messageId)}/translate`, {
          ...auth(),
          body: { target_language: targetLanguage },
        })
        .then((result) => result.text),
  }
}

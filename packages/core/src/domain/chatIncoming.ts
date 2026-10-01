/**
 * Merge one incoming broadcast into the visible list: reconcile optimistic sends first,
 * replace duplicates (idempotent on `message_id`, which is what makes reconnect history
 * replay and REST catch-up safe to re-deliver), append otherwise. Migrated verbatim from
 * `web/src/chatIncoming.ts` (TG-011); only import paths changed.
 */
import { reconcileOptimisticMessage } from './chatOptimistic'
import type { BroadcastMessage, DisplayMessage, MessageMotion } from './messageView'

export interface IncomingBroadcastResult {
  messages: DisplayMessage[]
  acknowledgedClientId: string
}

export function mergeIncomingBroadcast(
  messages: DisplayMessage[],
  incoming: BroadcastMessage,
  motion: MessageMotion,
): IncomingBroadcastResult {
  const reconciled = reconcileOptimisticMessage(messages, incoming)
  if (reconciled.matched) {
    return {
      messages: reconciled.messages,
      acknowledgedClientId: incoming.client_message_id || '',
    }
  }

  const duplicate = messages.some(
    (message) => message.type === 'broadcast' && message.message_id === incoming.message_id,
  )
  if (duplicate) {
    return {
      messages: messages.map((message) =>
        message.type === 'broadcast' && message.message_id === incoming.message_id
          ? {
              ...incoming,
              reactions: incoming.reactions || [],
              delivery_state: message.delivery_state,
              motion: message.motion,
            }
          : message,
      ),
      acknowledgedClientId: '',
    }
  }

  // TG-1208: a confirmed row lands before the trailing in-flight sends (failed ones keep
  // their place). The server orders
  // by arrival, and an own send the server has not acknowledged is newer than anything it
  // has already delivered — notably the history replay that a send made while the chat was
  // still opening used to end up below.
  let at = messages.length
  while (at > 0 && isUnconfirmed(messages[at - 1]!)) at -= 1
  const row: DisplayMessage = { ...incoming, reactions: incoming.reactions || [], motion }
  return {
    messages: [...messages.slice(0, at), row, ...messages.slice(at)],
    acknowledgedClientId: '',
  }
}

function isUnconfirmed(message: DisplayMessage): boolean {
  return (
    message.type === 'broadcast' && message.message_id.startsWith('pending:') && message.delivery_state === 'sending'
  )
}

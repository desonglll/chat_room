import { reconcileOptimisticMessage } from './chatOptimistic'
import type { BroadcastMessage, DisplayMessage, MessageMotion } from './types'

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

  return {
    messages: [...messages, { ...incoming, reactions: incoming.reactions || [], motion }],
    acknowledgedClientId: '',
  }
}

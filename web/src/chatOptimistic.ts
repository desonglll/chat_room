import type { BroadcastMessage, DisplayMessage, ReplyPreview, RoomMember } from './types'

interface OptimisticMessageInput {
  clientMessageId: string
  content: string
  replyTo: string
  currentUserId: string
  participants: RoomMember[]
  messages: DisplayMessage[]
  timestamp?: string
}

function replyPreview(messages: DisplayMessage[], messageId: string): ReplyPreview | null {
  if (!messageId) return null
  const target = messages.find(
    (message): message is BroadcastMessage => message.type === 'broadcast' && message.message_id === messageId,
  )
  if (!target) return null
  return {
    message_id: target.message_id,
    sender: target.sender,
    content: target.recalled_at ? '' : target.content,
    attachment_file_name: target.recalled_at ? null : target.attachment?.file_name || null,
    recalled: Boolean(target.recalled_at),
  }
}

export function createOptimisticMessage(input: OptimisticMessageInput): BroadcastMessage {
  const sender = input.participants.find((member) => member.user_id === input.currentUserId)
  return {
    type: 'broadcast',
    message_id: `pending:${input.clientMessageId}`,
    client_message_id: input.clientMessageId,
    sender_id: input.currentUserId,
    sender: sender?.username || '你',
    sender_avatar: sender?.avatar_emoji || '',
    content: input.content,
    attachment: null,
    reply_to: replyPreview(input.messages, input.replyTo),
    recalled_at: null,
    edited_at: null,
    timestamp: input.timestamp || new Date().toISOString(),
    favorite_id: null,
    forwarded_from: null,
    reactions: [],
    delivery_state: 'sending',
    motion: 'outgoing',
  }
}

export function reconcileOptimisticMessage(
  messages: DisplayMessage[],
  incoming: BroadcastMessage,
): { messages: DisplayMessage[]; matched: boolean } {
  if (!incoming.client_message_id) return { messages, matched: false }
  const index = messages.findIndex(
    (message) => message.type === 'broadcast' && message.client_message_id === incoming.client_message_id,
  )
  if (index < 0) return { messages, matched: false }
  const next = [...messages]
  next[index] = { ...incoming, delivery_state: 'sent', motion: 'none' }
  return { messages: next, matched: true }
}

export function updateDeliveryState(
  messages: DisplayMessage[],
  clientMessageId: string,
  state: 'sending' | 'failed',
): DisplayMessage[] {
  return messages.map((message) =>
    message.type === 'broadcast' && message.client_message_id === clientMessageId
      ? { ...message, delivery_state: state, motion: 'none' }
      : message,
  )
}

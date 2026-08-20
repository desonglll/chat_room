import type { DisplayMessage, MessageReaction } from './types'

export const QUICK_REACTIONS = ['👍', '❤️', '😂', '😮', '😢', '👏'] as const

export interface MessageReactionEvent {
  message_id: string
  emoji: string
  user_id: string
  active: boolean
}

export function applyMessageReaction(messages: DisplayMessage[], event: MessageReactionEvent): DisplayMessage[] {
  if (!messages.some((message) => message.type === 'broadcast' && message.message_id === event.message_id)) {
    return messages
  }
  return messages.map((message) => {
    if (message.type !== 'broadcast' || message.message_id !== event.message_id) return message
    const reactions = message.reactions || []
    const current = reactions.find((reaction) => reaction.emoji === event.emoji)
    const users = new Set(current?.user_ids || [])
    if (event.active) users.add(event.user_id)
    else users.delete(event.user_id)

    const updated: MessageReaction[] = current
      ? reactions
          .map((reaction) => (reaction.emoji === event.emoji ? { ...reaction, user_ids: [...users] } : reaction))
          .filter((reaction) => reaction.user_ids.length > 0)
      : event.active
        ? [...reactions, { emoji: event.emoji, user_ids: [event.user_id] }]
        : reactions
    return { ...message, reactions: updated }
  })
}

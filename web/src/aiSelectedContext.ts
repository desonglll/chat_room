import type { BroadcastMessage, DisplayMessage } from './types'

export interface AiSelectedMessage {
  messageId: string
  sender: string
  preview: string
  sentAt: string
}

export function selectedAiMessages(messages: DisplayMessage[], messageIds: string[]): AiSelectedMessage[] {
  const byId = new Map(
    messages
      .filter((message): message is BroadcastMessage => message.type === 'broadcast' && !message.recalled_at)
      .map((message) => [message.message_id, message]),
  )
  return messageIds.flatMap((messageId) => {
    const message = byId.get(messageId)
    if (!message) return []
    const preview = message.content.trim() || (message.attachment ? `[附件] ${message.attachment.file_name}` : '[消息]')
    return [{ messageId, sender: message.sender, preview, sentAt: message.timestamp }]
  })
}

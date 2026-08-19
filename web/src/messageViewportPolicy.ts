interface ViewportMessage {
  message_id: string
  sender_id: string | null
  recalled_at?: string | null
}

export function firstUnreadMessageId(messages: ViewportMessage[], unreadCount: number, currentUserId: string): string {
  const count = Math.max(0, Math.floor(unreadCount))
  if (!count) return ''

  const incoming = messages.filter((message) => message.sender_id !== currentUserId && !message.recalled_at)
  return incoming[Math.max(0, incoming.length - count)]?.message_id || ''
}

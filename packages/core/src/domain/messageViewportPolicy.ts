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

interface MessageStartPosition {
  containerTop: number
  currentScrollTop: number
  messageTop: number
  topGap?: number
}

interface ViewportPosition {
  scrollTop: number
  clientHeight: number
  scrollHeight: number
}

export function isViewportNearBottom(position: ViewportPosition, threshold = 72): boolean {
  return position.scrollHeight - position.clientHeight - position.scrollTop <= threshold
}

export function messageStartScrollTop(position: MessageStartPosition): number {
  return Math.max(0, position.currentScrollTop + position.messageTop - position.containerTop - (position.topGap ?? 20))
}

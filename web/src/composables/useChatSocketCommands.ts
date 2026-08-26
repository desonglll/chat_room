import type { Ref } from 'vue'
import { createOptimisticMessage, updateDeliveryState } from '../chatOptimistic'
import { createRandomUuid } from '../randomUuid'
import type { BroadcastMessage, DisplayMessage, RoomMember } from '../types'

interface ChatSocketCommandOptions {
  socket: () => WebSocket | null
  authenticated: () => boolean
  messages: Ref<DisplayMessage[]>
  participants: Ref<RoomMember[]>
  currentUserId: Ref<string>
  deliveryTimers: Map<string, number>
  clearDeliveryTimer: (clientMessageId: string) => void
}

export function useChatSocketCommands(options: ChatSocketCommandOptions) {
  let typingTimer: number | undefined
  let pendingTyping = ''

  function transmitOptimistic(message: BroadcastMessage): boolean {
    const socket = options.socket()
    if (!message.client_message_id || !options.authenticated() || socket?.readyState !== WebSocket.OPEN) return false
    socket.send(
      JSON.stringify({
        type: 'message',
        content: message.content,
        reply_to: message.reply_to?.message_id || undefined,
        client_message_id: message.client_message_id,
      }),
    )
    options.clearDeliveryTimer(message.client_message_id)
    options.deliveryTimers.set(
      message.client_message_id,
      window.setTimeout(() => {
        options.messages.value = updateDeliveryState(
          options.messages.value,
          message.client_message_id as string,
          'failed',
        )
        options.deliveryTimers.delete(message.client_message_id as string)
      }, 10_000),
    )
    return true
  }

  function send(content: string, replyTo = ''): boolean {
    const normalized = content.trim()
    if (!normalized || !options.authenticated() || options.socket()?.readyState !== WebSocket.OPEN) return false
    const clientMessageId = createRandomUuid()
    const optimistic = createOptimisticMessage({
      clientMessageId,
      content: normalized,
      replyTo,
      currentUserId: options.currentUserId.value,
      participants: options.participants.value,
      messages: options.messages.value,
    })
    options.messages.value.push(optimistic)
    transmitOptimistic(optimistic)
    sendTyping('')
    return true
  }

  function retry(messageId: string): boolean {
    const pending = options.messages.value.find(
      (message): message is BroadcastMessage =>
        message.type === 'broadcast' && message.message_id === messageId && message.delivery_state === 'failed',
    )
    if (!pending?.client_message_id || !options.authenticated() || options.socket()?.readyState !== WebSocket.OPEN)
      return false
    options.messages.value = updateDeliveryState(options.messages.value, pending.client_message_id, 'sending')
    return transmitOptimistic(pending)
  }

  function sendCommand(command: Record<string, unknown>): boolean {
    const socket = options.socket()
    if (!options.authenticated() || socket?.readyState !== WebSocket.OPEN) return false
    socket.send(JSON.stringify(command))
    return true
  }

  function edit(messageId: string, content: string): boolean {
    const normalized = content.trim()
    if (!messageId || !normalized || !sendCommand({ type: 'edit', message_id: messageId, content: normalized }))
      return false
    sendTyping('')
    return true
  }

  function flushTyping(): void {
    typingTimer = undefined
    sendCommand({ type: 'typing', content: pendingTyping })
  }

  function sendTyping(content: string): void {
    pendingTyping = content.slice(0, 512)
    if (!typingTimer) typingTimer = window.setTimeout(flushTyping, 90)
  }

  function clearTyping(): void {
    window.clearTimeout(typingTimer)
    typingTimer = undefined
    pendingTyping = ''
  }

  return {
    clearTyping,
    edit,
    markRead: (messageId: string) => Boolean(messageId) && sendCommand({ type: 'read', message_id: messageId }),
    poke: (targetUserId: string) =>
      Boolean(targetUserId) && sendCommand({ type: 'poke', target_user_id: targetUserId }),
    react: (messageId: string, emoji: string, active: boolean) =>
      Boolean(messageId && emoji) && sendCommand({ type: 'reaction', message_id: messageId, emoji, active }),
    recall: (messageId: string) => Boolean(messageId) && sendCommand({ type: 'recall', message_id: messageId }),
    retry,
    send,
    sendTyping,
  }
}

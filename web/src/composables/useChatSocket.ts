import { computed, onBeforeUnmount, ref } from 'vue'
import { mergeIncomingBroadcast } from '../chatIncoming'
import { createOptimisticMessage, updateDeliveryState } from '../chatOptimistic'
import { AUTH_ERRORS, readableSystemMessage, type ServerMessage } from '../chatProtocol'
import { classifyMessageMotion, classifySystemMotion } from '../messageMotion'
import { applyMessageReaction } from '../messageReactions'
import { createRandomUuid } from '../randomUuid'
import { useChatUploadMessages } from './useChatUploadMessages'
import type { BroadcastMessage, ChatStatus, DisplayMessage, ReadReceipt, Room, RoomMember, TypingDraft } from '../types'

interface ReconnectTarget {
  room: Room
  token: string
  userId: string
  password: string
}

export function useChatSocket(onSystemEvent?: (content: string) => void) {
  const status = ref<ChatStatus>('idle')
  const error = ref('')
  const messages = ref<DisplayMessage[]>([])
  const historyReady = ref(false)
  const members = ref<RoomMember[]>([])
  const participants = ref<RoomMember[]>([])
  const readReceipts = ref<ReadReceipt[]>([])
  const typingDrafts = ref<TypingDraft[]>([])
  const currentUserId = ref('')
  const pokedAt = ref(0)
  const authFailureReason = ref('')
  let socket: WebSocket | null = null
  let handshakeTimer: number | undefined
  let reconnectTimer: number | undefined
  let reconnectTarget: ReconnectTarget | null = null
  let reconnectAttempt = 0
  let reconnectEnabled = false
  let systemMessageId = 0
  let typingTimer: number | undefined
  let pendingTyping = ''
  const typingExpiry = new Map<string, number>()
  const deliveryTimers = new Map<string, number>()
  const uploadMessages = useChatUploadMessages(messages, (message) => appendBroadcast(message, false))
  const authenticated = computed(() => status.value === 'online')
  const statusLabel = computed(
    () =>
      ({
        idle: '未连接',
        connecting: '连接中',
        online: '已连接',
        offline: '已断开',
        failed: '认证失败',
      })[status.value],
  )

  function clearHandshakeTimer(): void {
    window.clearTimeout(handshakeTimer)
    handshakeTimer = undefined
  }

  function clearReconnectTimer(): void {
    window.clearTimeout(reconnectTimer)
    reconnectTimer = undefined
  }

  function clearDeliveryTimer(clientMessageId: string): void {
    window.clearTimeout(deliveryTimers.get(clientMessageId))
    deliveryTimers.delete(clientMessageId)
  }

  function failPendingMessages(): void {
    for (const message of messages.value) {
      if (message.type !== 'broadcast' || message.delivery_state !== 'sending' || !message.client_message_id) continue
      clearDeliveryTimer(message.client_message_id)
      messages.value = updateDeliveryState(messages.value, message.client_message_id, 'failed')
    }
  }

  function clearDeliveryTimers(): void {
    for (const timer of deliveryTimers.values()) window.clearTimeout(timer)
    deliveryTimers.clear()
  }

  function close({ preserveMessages = false } = {}): void {
    clearHandshakeTimer()
    clearReconnectTimer()
    reconnectEnabled = false
    reconnectTarget = null
    reconnectAttempt = 0
    if (socket) {
      socket.onclose = null
      socket.close()
    }
    socket = null
    status.value = 'idle'
    error.value = ''
    authFailureReason.value = ''
    if (preserveMessages) failPendingMessages()
    else {
      clearDeliveryTimers()
      messages.value = []
    }
    historyReady.value = false
    members.value = []
    participants.value = []
    readReceipts.value = []
    clearTypingDrafts()
  }

  function appendSystem(content: string): void {
    messages.value.push({
      type: 'system',
      key: `system-${++systemMessageId}`,
      content: readableSystemMessage(content),
      motion: classifySystemMotion(historyReady.value),
    })
  }

  function handleMessage(message: ServerMessage): void {
    if (message.type === 'auth_ok') {
      clearHandshakeTimer()
      if (reconnectAttempt === 0) messages.value = []
      historyReady.value = false
      members.value = message.members || []
      participants.value = message.participants || []
      readReceipts.value = message.read_receipts || []
      status.value = 'online'
      error.value = ''
      authFailureReason.value = ''
      reconnectAttempt = 0
      return
    }
    if (message.type === 'history_complete') {
      historyReady.value = true
      return
    }
    if (message.type === 'auth_fail') {
      clearHandshakeTimer()
      status.value = 'failed'
      error.value = AUTH_ERRORS[message.reason] || message.reason || '认证失败'
      authFailureReason.value = message.reason
      reconnectEnabled = false
      socket?.close()
      return
    }
    if (message.type === 'broadcast') {
      appendBroadcast(message)
      return
    }
    if (message.type === 'read_receipt') {
      const receipt: ReadReceipt = {
        user_id: message.user_id,
        username: message.username,
        message_id: message.message_id,
      }
      readReceipts.value = [...readReceipts.value.filter((item) => item.user_id !== receipt.user_id), receipt]
      return
    }
    if (message.type === 'message_recalled') {
      messages.value = messages.value.map((item) => {
        if (item.type !== 'broadcast') return item
        const recalledReply =
          item.reply_to?.message_id === message.message_id
            ? { ...item.reply_to, content: '', attachment_file_name: null, recalled: true }
            : item.reply_to
        if (item.message_id !== message.message_id) return { ...item, reply_to: recalledReply }
        // The sender keeps seeing their own text/attachment locally so they can re-edit
        // the recalled draft; everyone else's copy is blanked like before.
        const isOwn = item.sender_id === currentUserId.value
        return {
          ...item,
          content: isOwn ? item.content : '',
          attachment: isOwn ? item.attachment : null,
          recalled_at: message.recalled_at,
          reply_to: recalledReply,
        }
      })
      return
    }
    if (message.type === 'message_edited') {
      messages.value = messages.value.map((item) => {
        if (item.type !== 'broadcast') return item
        const replyTo =
          item.reply_to?.message_id === message.message_id
            ? { ...item.reply_to, content: message.content, recalled: false }
            : item.reply_to
        return item.message_id === message.message_id
          ? {
              ...item,
              content: message.content,
              edited_at: message.edited_at,
              recalled_at: null,
              reply_to: replyTo,
            }
          : { ...item, reply_to: replyTo }
      })
      return
    }
    if (message.type === 'reaction_changed') {
      messages.value = applyMessageReaction(messages.value, message)
      return
    }
    if (message.type === 'typing') {
      applyTyping(message)
      return
    }
    if (message.type === 'presence') {
      applyPresence(message.members, message.participants)
      return
    }

    const content = message.content || ''
    if (message.members) members.value = message.members
    if (message.participants) participants.value = message.participants
    const poke = content.match(/^poke:([^:]+):([^:]+)$/)
    if (poke) {
      applyPoke(poke[1], poke[2])
      return
    }
    appendSystem(content)
    onSystemEvent?.(content)
  }

  function resolveName(userId: string): string {
    return participants.value.find((member) => member.user_id === userId)?.username || '某人'
  }

  function applyPoke(fromUserId: string, targetUserId: string): void {
    appendSystem(
      `${resolveName(fromUserId)} 拍了拍 ${targetUserId === currentUserId.value ? '你' : resolveName(targetUserId)}`,
    )
    if (targetUserId === currentUserId.value) pokedAt.value = Date.now()
  }

  function appendBroadcast(message: BroadcastMessage, _showBrowserNotification = true): void {
    const result = mergeIncomingBroadcast(
      messages.value,
      message,
      classifyMessageMotion(historyReady.value, message.sender_id, currentUserId.value),
    )
    if (result.acknowledgedClientId) clearDeliveryTimer(result.acknowledgedClientId)
    messages.value = result.messages
  }

  function prependHistory(older: BroadcastMessage[]): void {
    const existing = new Set(messages.value.filter((item) => item.type === 'broadcast').map((item) => item.message_id))
    const fresh = older
      .filter((item) => !existing.has(item.message_id))
      .map((item) => ({ ...item, motion: 'none' as const }))
    if (fresh.length) messages.value = [...fresh, ...messages.value]
  }

  function clearTypingDrafts(): void {
    window.clearTimeout(typingTimer)
    typingTimer = undefined
    pendingTyping = ''
    for (const timer of typingExpiry.values()) window.clearTimeout(timer)
    typingExpiry.clear()
    typingDrafts.value = []
  }

  function applyTyping(message: { user_id?: string; username?: string; content: string }): void {
    const userId = message.user_id
    if (!userId || !message.username || userId === currentUserId.value) return
    window.clearTimeout(typingExpiry.get(userId))
    typingDrafts.value = message.content
      ? [
          ...typingDrafts.value.filter((draft) => draft.user_id !== userId),
          { user_id: userId, username: message.username, content: message.content },
        ]
      : typingDrafts.value.filter((draft) => draft.user_id !== userId)
    if (!message.content) {
      typingExpiry.delete(userId)
      return
    }
    typingExpiry.set(
      userId,
      window.setTimeout(() => {
        typingDrafts.value = typingDrafts.value.filter((draft) => draft.user_id !== userId)
        typingExpiry.delete(userId)
      }, 4000),
    )
  }

  function applyPresence(nextMembers: RoomMember[], nextParticipants: RoomMember[]): void {
    members.value = nextMembers
    participants.value = nextParticipants
    const avatars = new Map(nextParticipants.map((member) => [member.user_id, member.avatar_emoji]))
    messages.value = messages.value.map((item) =>
      item.type === 'broadcast' && item.sender_id
        ? { ...item, sender_avatar: avatars.get(item.sender_id) ?? item.sender_avatar }
        : item,
    )
  }

  function scheduleReconnect(): void {
    if (!reconnectEnabled || !reconnectTarget || reconnectTimer !== undefined) return
    const delay = Math.min(500 * 2 ** reconnectAttempt, 5_000)
    reconnectAttempt += 1
    reconnectTimer = window.setTimeout(() => {
      reconnectTimer = undefined
      if (reconnectTarget) openSocket(reconnectTarget)
    }, delay)
  }

  function openSocket(target: ReconnectTarget): void {
    const { room, token, userId, password } = target
    currentUserId.value = userId
    status.value = 'connecting'
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    const nextSocket = new WebSocket(`${protocol}//${window.location.host}/ws/${room.id}`)
    socket = nextSocket

    handshakeTimer = window.setTimeout(() => {
      if (socket === nextSocket && status.value === 'connecting') {
        error.value = '认证超时，请重试'
        nextSocket.close()
      }
    }, 10_000)

    nextSocket.onopen = () => {
      if (socket !== nextSocket) return
      const greeting = room.has_password ? { type: 'auth', token, password } : { type: 'join', token }
      nextSocket.send(JSON.stringify(greeting))
    }

    nextSocket.onmessage = (event: MessageEvent<string>) => {
      if (socket !== nextSocket) return
      try {
        handleMessage(JSON.parse(event.data) as ServerMessage)
      } catch {
        status.value = 'failed'
        error.value = '服务器返回了无效消息'
        reconnectEnabled = false
        nextSocket.close()
      }
    }

    nextSocket.onerror = () => {
      if (socket !== nextSocket) return
      clearHandshakeTimer()
      if (status.value !== 'online') {
        error.value = '无法连接聊天室'
      }
    }

    nextSocket.onclose = () => {
      if (socket !== nextSocket) return
      clearHandshakeTimer()
      socket = null
      failPendingMessages()
      if (status.value === 'online') {
        appendSystem('连接已断开')
        status.value = 'offline'
        scheduleReconnect()
      } else if (status.value === 'connecting' && reconnectEnabled) {
        status.value = 'offline'
        scheduleReconnect()
      }
    }
  }

  function connect(room: Room, token: string, userId: string, password: string): void {
    close()
    reconnectTarget = { room, token, userId, password }
    reconnectEnabled = true
    openSocket(reconnectTarget)
  }

  function send(content: string, replyTo = ''): boolean {
    const normalized = content.trim()
    if (!normalized || !authenticated.value || socket?.readyState !== WebSocket.OPEN) return false
    const clientMessageId = createRandomUuid()
    const optimistic = createOptimisticMessage({
      clientMessageId,
      content: normalized,
      replyTo,
      currentUserId: currentUserId.value,
      participants: participants.value,
      messages: messages.value,
    })
    messages.value.push(optimistic)
    transmitOptimistic(optimistic)
    sendTyping('')
    return true
  }

  function transmitOptimistic(message: BroadcastMessage): boolean {
    if (!message.client_message_id || !authenticated.value || socket?.readyState !== WebSocket.OPEN) return false
    socket.send(
      JSON.stringify({
        type: 'message',
        content: message.content,
        reply_to: message.reply_to?.message_id || undefined,
        client_message_id: message.client_message_id,
      }),
    )
    clearDeliveryTimer(message.client_message_id)
    deliveryTimers.set(
      message.client_message_id,
      window.setTimeout(() => {
        messages.value = updateDeliveryState(messages.value, message.client_message_id as string, 'failed')
        deliveryTimers.delete(message.client_message_id as string)
      }, 10_000),
    )
    return true
  }

  function retry(messageId: string): boolean {
    const pending = messages.value.find(
      (message): message is BroadcastMessage =>
        message.type === 'broadcast' && message.message_id === messageId && message.delivery_state === 'failed',
    )
    if (!pending?.client_message_id || !authenticated.value || socket?.readyState !== WebSocket.OPEN) return false
    messages.value = updateDeliveryState(messages.value, pending.client_message_id, 'sending')
    return transmitOptimistic(pending)
  }

  function edit(messageId: string, content: string): boolean {
    const normalized = content.trim()
    if (!messageId || !normalized || !authenticated.value || socket?.readyState !== WebSocket.OPEN) return false
    socket.send(JSON.stringify({ type: 'edit', message_id: messageId, content: normalized }))
    sendTyping('')
    return true
  }

  function flushTyping(): void {
    typingTimer = undefined
    if (!authenticated.value || socket?.readyState !== WebSocket.OPEN) return
    socket.send(JSON.stringify({ type: 'typing', content: pendingTyping }))
  }

  function sendTyping(content: string): void {
    pendingTyping = content.slice(0, 512)
    if (!typingTimer) typingTimer = window.setTimeout(flushTyping, 90)
  }

  function markRead(messageId: string): boolean {
    if (!messageId || !authenticated.value || socket?.readyState !== WebSocket.OPEN) return false
    socket.send(JSON.stringify({ type: 'read', message_id: messageId }))
    return true
  }

  function recall(messageId: string): boolean {
    if (!messageId || !authenticated.value || socket?.readyState !== WebSocket.OPEN) return false
    socket.send(JSON.stringify({ type: 'recall', message_id: messageId }))
    return true
  }

  function poke(targetUserId: string): boolean {
    if (!targetUserId || !authenticated.value || socket?.readyState !== WebSocket.OPEN) return false
    socket.send(JSON.stringify({ type: 'poke', target_user_id: targetUserId }))
    return true
  }

  function react(messageId: string, emoji: string, active: boolean): boolean {
    if (!messageId || !emoji || !authenticated.value || socket?.readyState !== WebSocket.OPEN) return false
    socket.send(JSON.stringify({ type: 'reaction', message_id: messageId, emoji, active }))
    return true
  }

  onBeforeUnmount(() => close())

  return {
    authFailureReason,
    authenticated,
    currentUserId,
    error,
    historyReady,
    members,
    messages,
    participants,
    readReceipts,
    status,
    statusLabel,
    typingDrafts,
    pokedAt,
    appendBroadcast,
    ...uploadMessages,
    prependHistory,
    close,
    connect,
    edit,
    markRead,
    recall,
    react,
    retry,
    poke,
    send,
    sendTyping,
  }
}

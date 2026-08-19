import { computed, onBeforeUnmount, ref } from 'vue'
import type { BroadcastMessage, ChatStatus, DisplayMessage, ReadReceipt, Room, RoomMember, TypingDraft } from '../types'

type ServerMessage =
  | {
      type: 'auth_ok'
      room_name: string
      members?: RoomMember[]
      participants?: RoomMember[]
      read_receipts?: ReadReceipt[]
    }
  | { type: 'auth_fail'; reason: string }
  | { type: 'history_complete' }
  | BroadcastMessage
  | { type: 'read_receipt'; user_id: string; username: string; message_id: string }
  | { type: 'message_recalled'; message_id: string; recalled_at: string }
  | { type: 'message_edited'; message_id: string; content: string; edited_at: string }
  | { type: 'typing'; user_id?: string; username?: string; content: string }
  | { type: 'presence'; members: RoomMember[]; participants: RoomMember[] }
  | { type: 'system'; content: string; members?: RoomMember[]; participants?: RoomMember[] }

interface ReconnectTarget {
  room: Room
  token: string
  userId: string
  password: string
}

const AUTH_ERRORS: Record<string, string> = {
  'wrong password': '房间密码错误',
  'room not found': '聊天室不存在',
  'authentication timeout': '认证超时，请重试',
  'login required': '请重新登录',
  'authentication unavailable': '暂时无法验证登录状态',
  'password too long': '房间密码过长',
  'membership required': '请先申请加入聊天室',
  'membership pending': '加入申请正在等待管理员审核',
  'invalid json': '认证请求无效',
}

function readableSystemMessage(content: string): string {
  const joined = content.match(/^(.*) joined the room$/)
  if (joined) return `${joined[1]} 加入了聊天室`
  const left = content.match(/^(.*) left the room$/)
  if (left) return `${left[1]} 离开了聊天室`
  const renamed = content.match(/^room renamed to (.*)$/)
  if (renamed) return `聊天室已重命名为 ${renamed[1]}`
  if (content === 'room deleted') return '聊天室已被删除'
  if (content === 'room password changed') return '聊天室密码已更改，请重新加入'
  if (content === 'message history is temporarily unavailable') return '暂时无法读取历史消息'
  const failed = content.match(/^message from (.*) was not saved or broadcast$/)
  if (failed) return `${failed[1]} 的消息保存失败`
  return content
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
    if (!preserveMessages) messages.value = []
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
    })
  }

  function handleMessage(message: ServerMessage): void {
    if (message.type === 'auth_ok') {
      clearHandshakeTimer()
      messages.value = []
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
    const duplicate = messages.value.some((item) => item.type === 'broadcast' && item.message_id === message.message_id)
    if (!duplicate) {
      messages.value.push(message)
    }
  }

  function prependHistory(older: BroadcastMessage[]): void {
    const existing = new Set(messages.value.filter((item) => item.type === 'broadcast').map((item) => item.message_id))
    const fresh = older.filter((item) => !existing.has(item.message_id))
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
      clearHandshakeTimer()
      if (status.value !== 'online') {
        error.value = '无法连接聊天室'
      }
    }

    nextSocket.onclose = () => {
      if (socket !== nextSocket) return
      clearHandshakeTimer()
      socket = null
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
    socket.send(
      JSON.stringify({
        type: 'message',
        content: normalized,
        reply_to: replyTo || undefined,
      }),
    )
    sendTyping('')
    return true
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
    prependHistory,
    close,
    connect,
    edit,
    markRead,
    recall,
    poke,
    send,
    sendTyping,
  }
}

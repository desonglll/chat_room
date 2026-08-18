import { computed, onBeforeUnmount, ref } from 'vue'
import type { BroadcastMessage, ChatStatus, DisplayMessage, Room } from '../types'

type ServerMessage =
  | { type: 'auth_ok'; room_name: string }
  | { type: 'auth_fail'; reason: string }
  | BroadcastMessage
  | { type: 'system'; content: string }

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
  const currentUserId = ref('')
  let socket: WebSocket | null = null
  let handshakeTimer: number | undefined
  let reconnectTimer: number | undefined
  let reconnectTarget: ReconnectTarget | null = null
  let reconnectAttempt = 0
  let reconnectEnabled = false
  let systemMessageId = 0

  const authenticated = computed(() => status.value === 'online')
  const statusLabel = computed(() => ({
    idle: '未连接',
    connecting: '连接中',
    online: '已连接',
    offline: '已断开',
    failed: '认证失败',
  })[status.value])

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
    if (!preserveMessages) messages.value = []
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
      status.value = 'online'
      error.value = ''
      reconnectAttempt = 0
      return
    }
    if (message.type === 'auth_fail') {
      clearHandshakeTimer()
      status.value = 'failed'
      error.value = AUTH_ERRORS[message.reason] || message.reason || '认证失败'
      reconnectEnabled = false
      socket?.close()
      return
    }
    if (message.type === 'broadcast') {
      appendBroadcast(message)
      return
    }

    const content = message.content || ''
    appendSystem(content)
    onSystemEvent?.(content)
  }

  function appendBroadcast(message: BroadcastMessage): void {
    const duplicate = messages.value.some(
      (item) => item.type === 'broadcast' && item.message_id === message.message_id,
    )
    if (!duplicate) messages.value.push(message)
  }

  function scheduleReconnect(): void {
    if (!reconnectEnabled || !reconnectTarget || reconnectTimer !== undefined) return
    const delay = Math.min(500 * (2 ** reconnectAttempt), 5_000)
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
      const greeting = room.has_password
        ? { type: 'auth', token, password }
        : { type: 'join', token }
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

  function send(content: string): boolean {
    const normalized = content.trim()
    if (!normalized || !authenticated.value || socket?.readyState !== WebSocket.OPEN) return false
    socket.send(JSON.stringify({ type: 'message', content: normalized }))
    return true
  }

  onBeforeUnmount(() => close())

  return {
    authenticated,
    currentUserId,
    error,
    messages,
    status,
    statusLabel,
    appendBroadcast,
    close,
    connect,
    send,
  }
}

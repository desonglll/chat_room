/**
 * The chat WebSocket client: handshake, per-frame-type subscriptions, reconnect with
 * backoff, and missed-message catch-up after reconnect. Everything platform-shaped is
 * injected (`CoreSocketFactory`, `CoreClock`), so the whole lifecycle is testable against
 * a fake socket and a fake clock and portable off-DOM.
 *
 * Protocol facts this file encodes (TG-007 frozen interface):
 * - handshake: `auth {token, password}` when a password is supplied, else `join {token}`;
 *   the server replies `auth_ok` (then history replay, then `history_complete`) or
 *   `auth_fail` (terminal — reconnecting with the same credentials cannot succeed);
 * - unknown server frames are ignored (codec returns null), the connection stays up;
 * - reconnect replays recent history server-side; the catch-up hook covers the tail the
 *   replay window may miss, and both paths converge on ordinary `broadcast` frames whose
 *   downstream merge (`domain/chatIncoming`) is idempotent on `message_id`.
 */
import { compareInstants } from '../domain/instant'
import type {
  ClientFrame,
  CoreClock,
  CoreSocket,
  CoreSocketFactory,
  CoreTimerHandle,
  ServerFrame,
  StoredMessage,
} from '../types'
import { storedMessageToBroadcast } from '../domain/messageView'
import type { BackoffOptions } from './backoff'
import { reconnectDelayMs } from './backoff'
import { encodeClientFrame, parseServerFrame } from './frameCodec'

export type ChatSocketStatus = 'idle' | 'connecting' | 'online' | 'offline' | 'failed'

/** Cursor of the newest broadcast this client has seen (mirrors `MessageCursor`). */
export interface BroadcastCursor {
  timestamp: string
  messageId: string
}

export interface ChatSocketOptions {
  /** Full socket URL, e.g. built by the host from `/ws/:room_id`. */
  url: string
  token: string
  /** Chat password; presence switches the greeting from `join` to `auth`. */
  password?: string
  createSocket: CoreSocketFactory
  clock: CoreClock
  /**
   * Missed-message catch-up: called after the `history_complete` of a RE-connect with the
   * newest cursor seen before the drop. Return the messages newer than the cursor (REST
   * `GET /api/chats/:id/messages`); they are re-emitted as `broadcast` frames.
   */
  fetchMissed?: (cursor: BroadcastCursor) => Promise<StoredMessage[]>
  handshakeTimeoutMs?: number
  backoff?: BackoffOptions
}

type FrameHandler = (frame: ServerFrame) => void

export interface ChatSocket {
  connect(): void
  /** Intentional shutdown: closes the socket and disables reconnection. */
  close(): void
  /** Sends when online; returns false (and sends nothing) otherwise. */
  send(frame: ClientFrame): boolean
  on<T extends ServerFrame['type']>(type: T, handler: (frame: Extract<ServerFrame, { type: T }>) => void): () => void
  onAnyFrame(handler: FrameHandler): () => void
  onStatus(handler: (status: ChatSocketStatus) => void): () => void
  status(): ChatSocketStatus
  lastBroadcastCursor(): BroadcastCursor | null
  /** Reconnect attempts scheduled since the last successful handshake. */
  reconnectAttempts(): number
}

export function createChatSocket(options: ChatSocketOptions): ChatSocket {
  const clock = options.clock
  const handshakeTimeoutMs = options.handshakeTimeoutMs ?? 10_000

  let socket: CoreSocket | null = null
  let currentStatus: ChatSocketStatus = 'idle'
  let reconnectEnabled = false
  let attempt = 0
  let everAuthenticated = false
  let cursor: BroadcastCursor | null = null
  let cursorAtDisconnect: BroadcastCursor | null = null
  let handshakeTimer: CoreTimerHandle | null = null
  let reconnectTimer: CoreTimerHandle | null = null

  const typeHandlers = new Map<string, Set<FrameHandler>>()
  const anyHandlers = new Set<FrameHandler>()
  const statusHandlers = new Set<(status: ChatSocketStatus) => void>()

  function setStatus(next: ChatSocketStatus): void {
    if (currentStatus === next) return
    currentStatus = next
    for (const handler of [...statusHandlers]) handler(next)
  }

  function emit(frame: ServerFrame): void {
    if (frame.type === 'broadcast') {
      cursor = { timestamp: frame.timestamp, messageId: frame.message_id }
    }
    for (const handler of [...(typeHandlers.get(frame.type) ?? [])]) handler(frame)
    for (const handler of [...anyHandlers]) handler(frame)
  }

  function clearTimer(handle: CoreTimerHandle | null): null {
    if (handle !== null) clock.clearTimeout(handle)
    return null
  }

  function scheduleReconnect(): void {
    if (!reconnectEnabled || reconnectTimer !== null) return
    const delay = reconnectDelayMs(attempt, options.backoff ?? {})
    attempt += 1
    reconnectTimer = clock.setTimeout(() => {
      reconnectTimer = null
      openSocket()
    }, delay)
  }

  async function runCatchUp(since: BroadcastCursor): Promise<void> {
    if (!options.fetchMissed) return
    let missed: StoredMessage[]
    try {
      missed = await options.fetchMissed(since)
    } catch {
      // The next reconnect (or the server's own replay) gets another chance; swallowing
      // keeps a flaky REST call from tearing down a healthy socket.
      return
    }
    for (const message of missed) {
      if (message.id === since.messageId || compareInstants(message.created_at, since.timestamp) < 0) continue
      emit(storedMessageToBroadcast(message) as ServerFrame)
    }
  }

  function handleFrame(frame: ServerFrame): void {
    if (frame.type === 'auth_ok') {
      handshakeTimer = clearTimer(handshakeTimer)
      everAuthenticated = true
      attempt = 0
      setStatus('online')
    } else if (frame.type === 'auth_fail') {
      // Terminal: retrying with the same credentials cannot succeed.
      handshakeTimer = clearTimer(handshakeTimer)
      reconnectEnabled = false
    } else if (frame.type === 'history_complete') {
      const since = cursorAtDisconnect
      cursorAtDisconnect = null
      if (since) void runCatchUp(since)
    }
    emit(frame)
    if (frame.type === 'auth_fail') {
      setStatus('failed')
      socket?.close()
    }
  }

  function openSocket(): void {
    cursorAtDisconnect = everAuthenticated ? cursor : null
    setStatus('connecting')
    let next: ReturnType<CoreSocketFactory>
    try {
      next = options.createSocket(options.url)
    } catch {
      // TG-906: a factory that throws synchronously (bad URL, blocked by CSP, out of sockets)
      // is a failed attempt like any close — it must not leave the status stuck on
      // `connecting` with no retry scheduled.
      setStatus(reconnectEnabled ? 'offline' : 'idle')
      scheduleReconnect()
      return
    }
    socket = next
    next.onopen = () => {
      handshakeTimer = clearTimer(handshakeTimer)
      handshakeTimer = clock.setTimeout(() => {
        handshakeTimer = null
        next.close()
      }, handshakeTimeoutMs)
      const greeting: ClientFrame =
        options.password !== undefined && options.password !== ''
          ? { type: 'auth', token: options.token, password: options.password }
          : { type: 'join', token: options.token }
      next.send(encodeClientFrame(greeting))
    }
    next.onmessage = (data) => {
      if (socket !== next) return
      const frame = parseServerFrame(data)
      if (frame) handleFrame(frame)
    }
    next.onclose = () => {
      if (socket !== next) return
      socket = null
      handshakeTimer = clearTimer(handshakeTimer)
      if (currentStatus !== 'failed') {
        setStatus(reconnectEnabled ? 'offline' : 'idle')
        scheduleReconnect()
      }
    }
    next.onerror = () => {
      // The close event carries the lifecycle; errors alone change nothing.
    }
  }

  return {
    connect() {
      if (socket || reconnectTimer !== null) return
      reconnectEnabled = true
      attempt = 0
      openSocket()
    },
    close() {
      reconnectEnabled = false
      reconnectTimer = clearTimer(reconnectTimer)
      handshakeTimer = clearTimer(handshakeTimer)
      const current = socket
      socket = null
      current?.close()
      setStatus('idle')
    },
    send(frame) {
      if (!socket || currentStatus !== 'online') return false
      socket.send(encodeClientFrame(frame))
      return true
    },
    on(type, handler) {
      const handlers = typeHandlers.get(type) ?? new Set<FrameHandler>()
      typeHandlers.set(type, handlers)
      handlers.add(handler as FrameHandler)
      return () => handlers.delete(handler as FrameHandler)
    },
    onAnyFrame(handler) {
      anyHandlers.add(handler)
      return () => anyHandlers.delete(handler)
    },
    onStatus(handler) {
      statusHandlers.add(handler)
      return () => statusHandlers.delete(handler)
    },
    status: () => currentStatus,
    lastBroadcastCursor: () => cursor,
    reconnectAttempts: () => attempt,
  }
}

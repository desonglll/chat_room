// TG-011: the WS client lifecycle against an injected fake socket and fake clock —
// handshake dialect, subscription management, reconnect backoff (the frozen Vue curve),
// and missed-message catch-up after reconnect.
import { describe, expect, test } from 'bun:test'
import type { CoreClock, CoreSocket, ServerFrame, StoredMessage } from '../types'
import { reconnectDelayMs } from './backoff'
import { createChatSocket } from './chatSocket'

class FakeSocket implements CoreSocket {
  sent: string[] = []
  closed = false
  onopen: (() => void) | null = null
  onmessage: ((data: string) => void) | null = null
  onclose: ((info: { code: number; reason: string }) => void) | null = null
  onerror: (() => void) | null = null

  send(data: string): void {
    this.sent.push(data)
  }

  close(): void {
    if (this.closed) return
    this.closed = true
    this.onclose?.({ code: 1000, reason: '' })
  }

  open(): void {
    this.onopen?.()
  }

  receive(frame: object): void {
    this.onmessage?.(JSON.stringify(frame))
  }

  dropFromServer(): void {
    this.closed = true
    this.onclose?.({ code: 1006, reason: '' })
  }
}

interface PendingTimer {
  id: number
  callback: () => void
  delay: number
}

class FakeClock implements CoreClock {
  private timers: PendingTimer[] = []
  private nextId = 1
  nowMs = 0

  now(): number {
    return this.nowMs
  }

  setTimeout(callback: () => void, delayMs: number): unknown {
    const id = this.nextId++
    this.timers.push({ id, callback, delay: delayMs })
    return id
  }

  clearTimeout(handle: unknown): void {
    this.timers = this.timers.filter((timer) => timer.id !== handle)
  }

  pendingDelays(): number[] {
    return this.timers.map((timer) => timer.delay)
  }

  fireNext(): void {
    const timer = this.timers.shift()
    timer?.callback()
  }
}

function harness(
  options: {
    password?: string
    fetchMissed?: (cursor: { timestamp: string; messageId: string }) => Promise<StoredMessage[]>
  } = {},
) {
  const sockets: FakeSocket[] = []
  const clock = new FakeClock()
  const socket = createChatSocket({
    url: 'ws://test/ws/chat-1',
    token: 'tok',
    ...(options.password !== undefined ? { password: options.password } : {}),
    ...(options.fetchMissed ? { fetchMissed: options.fetchMissed } : {}),
    createSocket: () => {
      const next = new FakeSocket()
      sockets.push(next)
      return next
    },
    clock,
  })
  return { sockets, clock, socket, latest: () => sockets[sockets.length - 1]! }
}

const AUTH_OK = {
  type: 'auth_ok',
  room_name: 'general',
  members: [],
  participants: [],
  read_receipts: [],
  statuses: [],
}

const broadcastFrame = (id: string, timestamp: string) => ({
  type: 'broadcast',
  message_id: id,
  sender_id: null,
  sender: 'alice',
  sender_avatar: '',
  content: 'hello',
  attachment: null,
  reply_to: null,
  recalled_at: null,
  edited_at: null,
  timestamp,
  favorite_id: null,
  forwarded_from: null,
  reactions: [],
})

const stored = (id: string, created_at: string): StoredMessage => ({
  id,
  client_message_id: null,
  favorite_id: null,
  room_id: 'chat-1',
  sender_id: null,
  sender: 'alice',
  sender_avatar: '',
  content: 'missed',
  attachment: null,
  reply_to: null,
  recalled_at: null,
  edited_at: null,
  created_at,
  forwarded_from: null,
  reactions: [],
})

describe('chat socket', () => {
  test('greets with join when no password and auth when one is supplied', () => {
    const open = harness()
    open.socket.connect()
    open.latest().open()
    expect(open.latest().sent).toEqual(['{"type":"join","token":"tok"}'])

    const guarded = harness({ password: 'pw' })
    guarded.socket.connect()
    guarded.latest().open()
    expect(guarded.latest().sent).toEqual(['{"type":"auth","token":"tok","password":"pw"}'])
  })

  test('routes frames to per-type subscriptions and honors unsubscribe', () => {
    const { socket, latest } = harness()
    const seen: string[] = []
    const off = socket.on('typing', (frame) => seen.push(`typing:${frame.action}`))
    socket.on('presence', () => seen.push('presence'))
    socket.onAnyFrame((frame: ServerFrame) => seen.push(`any:${frame.type}`))
    socket.connect()
    latest().open()
    latest().receive(AUTH_OK)
    latest().receive({ type: 'typing', content: 'x', action: 'recording_voice' })
    off()
    latest().receive({ type: 'typing', content: 'x' })
    latest().receive({ type: 'unheard_of_frame' })

    expect(socket.status()).toBe('online')
    expect(seen).toEqual(['any:auth_ok', 'typing:recording_voice', 'any:typing', 'any:typing'])
  })

  test('sends only while online', () => {
    const { socket, latest } = harness()
    expect(socket.send({ type: 'read', message_id: 'm' })).toBe(false)
    socket.connect()
    latest().open()
    expect(socket.send({ type: 'read', message_id: 'm' })).toBe(false)
    latest().receive(AUTH_OK)
    expect(socket.send({ type: 'read', message_id: 'm' })).toBe(true)
    expect(latest().sent).toHaveLength(2)
  })

  test('reconnects on the frozen backoff curve and resets it after auth_ok', () => {
    const { socket, sockets, clock, latest } = harness()
    socket.connect()
    latest().open()
    latest().receive(AUTH_OK)

    for (let attempt = 0; attempt < 6; attempt += 1) {
      latest().dropFromServer()
      expect(socket.status()).toBe('offline')
      expect(clock.pendingDelays()).toEqual([reconnectDelayMs(attempt)])
      clock.fireNext()
    }
    // 500, 1000, 2000, 4000, then capped at 5000.
    expect(reconnectDelayMs(0)).toBe(500)
    expect(reconnectDelayMs(3)).toBe(4_000)
    expect(reconnectDelayMs(4)).toBe(5_000)
    expect(reconnectDelayMs(5)).toBe(5_000)

    latest().open()
    latest().receive(AUTH_OK)
    expect(socket.status()).toBe('online')
    expect(socket.reconnectAttempts()).toBe(0)
    latest().dropFromServer()
    expect(clock.pendingDelays()).toEqual([500])
    expect(sockets).toHaveLength(7)
  })

  test('an intentional close disables reconnection', () => {
    const { socket, clock, latest, sockets } = harness()
    socket.connect()
    latest().open()
    latest().receive(AUTH_OK)
    socket.close()
    expect(socket.status()).toBe('idle')
    expect(clock.pendingDelays()).toEqual([])
    expect(sockets).toHaveLength(1)
  })

  test('auth_fail is terminal: no reconnect is scheduled', () => {
    const { socket, clock, latest } = harness({ password: 'wrong' })
    const statuses: string[] = []
    socket.onStatus((status) => statuses.push(status))
    socket.connect()
    latest().open()
    latest().receive({ type: 'auth_fail', reason: 'wrong password' })
    expect(socket.status()).toBe('failed')
    expect(clock.pendingDelays()).toEqual([])
    expect(statuses).toEqual(['connecting', 'failed'])
  })

  test('a handshake that never answers times out into the reconnect path', () => {
    const { socket, clock, latest } = harness()
    socket.connect()
    latest().open()
    expect(clock.pendingDelays()).toEqual([10_000])
    clock.fireNext()
    expect(socket.status()).toBe('offline')
    expect(clock.pendingDelays()).toEqual([reconnectDelayMs(0)])
  })

  test('catches up missed messages after reconnect and re-emits them as broadcasts', async () => {
    const cursors: { timestamp: string; messageId: string }[] = []
    const { socket, clock, latest } = harness({
      fetchMissed: async (cursor) => {
        cursors.push(cursor)
        return [
          stored('m1', '2026-09-30T12:00:00Z'),
          stored('m2', '2026-09-30T12:00:05Z'),
          stored('m3', '2026-09-30T12:00:06Z'),
        ]
      },
    })
    const received: string[] = []
    socket.on('broadcast', (frame) => received.push(frame.message_id))
    socket.connect()
    latest().open()
    latest().receive(AUTH_OK)
    latest().receive(broadcastFrame('m1', '2026-09-30T12:00:00Z'))
    latest().receive({ type: 'history_complete' })
    expect(cursors).toEqual([]) // first connection: no catch-up

    latest().dropFromServer()
    clock.fireNext()
    latest().open()
    latest().receive(AUTH_OK)
    latest().receive({ type: 'history_complete' })
    await Promise.resolve()
    await Promise.resolve()

    expect(cursors).toEqual([{ timestamp: '2026-09-30T12:00:00Z', messageId: 'm1' }])
    // m1 (the cursor message) is filtered; m2/m3 arrive as ordinary broadcasts.
    expect(received).toEqual(['m1', 'm2', 'm3'])
    expect(socket.lastBroadcastCursor()).toEqual({ timestamp: '2026-09-30T12:00:06Z', messageId: 'm3' })
  })
})

describe('TG-906 a factory that throws', () => {
  test('goes offline and retries instead of sticking on connecting', () => {
    const clock = new FakeClock()
    const sockets: FakeSocket[] = []
    let failures = 1
    const statuses: string[] = []
    const socket = createChatSocket({
      url: 'ws://test/ws/chat-1',
      token: 'tok',
      createSocket: () => {
        if (failures > 0) {
          failures -= 1
          throw new Error('blocked')
        }
        const next = new FakeSocket()
        sockets.push(next)
        return next
      },
      clock,
    })
    socket.onStatus((status) => statuses.push(status))
    socket.connect()
    expect(statuses.at(-1)).toBe('offline')
    expect(clock.pendingDelays()).toHaveLength(1)
    clock.fireNext()
    expect(sockets).toHaveLength(1)
    expect(statuses.at(-1)).toBe('connecting')
  })
})

describe('TG-906 catch-up compares instants', () => {
  test('a missed message with a fractional timestamp after a whole-second cursor is replayed', async () => {
    const frames: ServerFrame[] = []
    const { socket, latest, clock } = harness({
      fetchMissed: async () => [stored('m2', '2026-10-01T09:00:00.500Z')],
    })
    socket.on('broadcast', (frame) => frames.push(frame))
    socket.connect()
    latest().open()
    latest().receive(AUTH_OK)
    latest().receive(broadcastFrame('m1', '2026-10-01T09:00:00Z'))
    latest().receive({ type: 'history_complete' })
    latest().dropFromServer()
    clock.fireNext()
    latest().open()
    latest().receive(AUTH_OK)
    latest().receive({ type: 'history_complete' })
    await new Promise((resolve) => setTimeout(resolve, 0))
    expect(frames.map((frame) => (frame as { message_id: string }).message_id)).toContain('m2')
  })
})

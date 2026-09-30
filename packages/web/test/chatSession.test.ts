/**
 * The chat wiring seam under fakes (harness in ./chatSessionHarness): socket lifecycle
 * → store fan-out, optimistic send → broadcast reconcile, reconnect catch-up, raw frames
 * and read cursors (TG-100). The draft-policy half lives in chatSessionDrafts.test.ts.
 */
import { describe, expect, test } from 'bun:test'
import { selectTimeline } from '@tg/core'
import { AUTH_OK, broadcastFrame, CHAT_ID, harness, ME, settle, storedMessage } from './chatSessionHarness'

describe('connection fan-out', () => {
  test('join handshake, auth_ok → presence, history_complete → historyReady', async () => {
    const { session, sockets, stores, online } = harness()
    await online()
    expect(sockets[0]!.sentFrames()[0]).toEqual({ type: 'join', token: 'token-1' })
    expect(session.status()).toBe('online')
    expect(stores.presence.getState().chats[CHAT_ID]?.members).toHaveLength(2)
    expect(stores.presence.getState().chats[CHAT_ID]?.statuses[ME]).toEqual({ kind: 'online' })
    expect(selectTimeline(CHAT_ID)(stores.message.getState()).historyReady).toBeTrue()
    session.stop()
    expect(sockets[0]!.closed).toBeTrue()
  })

  test('broadcast frames land in the timeline; replay after reconnect stays deduped', async () => {
    const { session, sockets, stores, online } = harness()
    await online()
    sockets[0]!.receive(broadcastFrame('m1', '2026-09-30T09:00:00Z', '你好'))
    sockets[0]!.receive(broadcastFrame('m1', '2026-09-30T09:00:00Z', '你好'))
    const timeline = selectTimeline(CHAT_ID)(stores.message.getState())
    expect(timeline.messages).toHaveLength(1)
    expect(timeline.messages[0]).toMatchObject({ type: 'broadcast', content: '你好', motion: 'incoming' })
    session.stop()
  })

  test('typing frames show and expire through the injected clock', async () => {
    const { session, sockets, clock, stores, online } = harness()
    await online()
    sockets[0]!.receive({
      type: 'typing',
      content: '草稿…',
      action: 'typing',
      user_id: 'user-other',
      username: 'other',
    })
    expect(stores.presence.getState().chats[CHAT_ID]?.typing).toHaveLength(1)
    clock.advance(7000)
    expect(stores.presence.getState().chats[CHAT_ID]?.typing).toHaveLength(0)
    session.stop()
  })
})

describe('sending', () => {
  test('optimistic append → WS frame → broadcast reconciles by client_message_id', async () => {
    const { session, sockets, stores, online } = harness()
    await online()
    expect(session.sendMessage('  hello  ')).toBeTrue()
    const pending = selectTimeline(CHAT_ID)(stores.message.getState()).messages
    expect(pending).toHaveLength(1)
    expect(pending[0]).toMatchObject({ content: 'hello', delivery_state: 'sending', sender: 'me' })
    const wire = sockets[0]!.sentFrames().find((frame) => frame.type === 'message')!
    expect(wire.content).toBe('hello')
    const clientMessageId = wire.client_message_id as string
    expect(clientMessageId).toMatch(/[0-9a-f-]{36}/)
    sockets[0]!.receive(
      broadcastFrame('m-server', '2026-09-30T09:01:00Z', 'hello', {
        sender_id: ME,
        sender: 'me',
        client_message_id: clientMessageId,
      }),
    )
    const after = selectTimeline(CHAT_ID)(stores.message.getState()).messages
    expect(after).toHaveLength(1)
    expect(after[0]).toMatchObject({ message_id: 'm-server', delivery_state: 'sent' })
    session.stop()
  })

  test('a send while offline is marked failed; empty and over-limit sends are refused', async () => {
    const { session, stores } = harness()
    session.start() // never opens
    expect(session.sendMessage('offline row')).toBeFalse()
    const rows = selectTimeline(CHAT_ID)(stores.message.getState()).messages
    expect(rows[0]).toMatchObject({ delivery_state: 'failed' })
    expect(session.sendMessage('   ')).toBeFalse()
    expect(session.sendMessage('长'.repeat(4097))).toBeFalse()
    expect(selectTimeline(CHAT_ID)(stores.message.getState()).messages).toHaveLength(1)
    session.stop()
  })
})

describe('reconnect catch-up', () => {
  test('after the reconnect handshake the REST page is merged, deduped by message_id', async () => {
    const missed = [
      storedMessage('m3', '2026-09-30T09:03:00Z', 'newest'),
      storedMessage('m2', '2026-09-30T09:02:00Z', 'missed'),
      storedMessage('m1', '2026-09-30T09:00:00Z', 'seen'),
    ]
    const { session, sockets, clock, stores, missedCalls, online } = harness({ missed })
    await online()
    sockets[0]!.receive(broadcastFrame('m1', '2026-09-30T09:00:00Z', 'seen'))
    sockets[0]!.dropFromServer()
    expect(session.status()).toBe('offline')
    clock.advance(500) // frozen backoff: first retry after 500 ms
    const reconnected = sockets[1]!
    reconnected.open()
    reconnected.receive(AUTH_OK)
    reconnected.receive({ type: 'history_complete' })
    await settle()
    expect(missedCalls).toHaveLength(1)
    const contents = selectTimeline(CHAT_ID)(stores.message.getState()).messages.map((row) =>
      row.type === 'broadcast' ? row.content : '',
    )
    expect(contents).toEqual(['seen', 'missed', 'newest'])
    session.stop()
  })
})

describe('raw frames and read cursors (TG-100)', () => {
  const readFrames = (socket: { sentFrames(): Array<Record<string, unknown>> }) =>
    socket.sentFrames().filter((frame) => frame.type === 'read')

  test('sendFrame goes out as-is; draft edits send no typing frames (the composer owns typing)', async () => {
    const { session, sockets, online } = harness()
    await online()
    expect(session.sendFrame({ type: 'recall', message_id: 'm1' })).toBeTrue()
    session.setDraftText('abc')
    const frames = sockets[0]!.sentFrames()
    expect(frames).toContainEqual({ type: 'recall', message_id: 'm1' })
    expect(frames.filter((frame) => frame.type === 'typing')).toHaveLength(0)
    session.stop()
  })

  test('auth_ok receipts and read_receipt frames become read cursors', async () => {
    const { session, sockets, stores } = harness()
    session.start()
    await settle()
    sockets[0]!.open()
    sockets[0]!.receive({ ...AUTH_OK, read_receipts: [{ user_id: 'user-other', username: 'other', message_id: 'm1' }] })
    expect(selectTimeline(CHAT_ID)(stores.message.getState()).readCursors).toEqual({ 'user-other': 'm1' })
    sockets[0]!.receive({ type: 'read_receipt', user_id: 'user-other', username: 'other', message_id: 'm2' })
    expect(selectTimeline(CHAT_ID)(stores.message.getState()).readCursors).toEqual({ 'user-other': 'm2' })
    session.stop()
  })

  test('the newest message is marked read on history completion and on each incoming arrival, once', async () => {
    const { session, sockets, stores } = harness()
    session.start()
    await settle()
    const socket = sockets[0]!
    socket.open()
    socket.receive(AUTH_OK)
    socket.receive(broadcastFrame('m1', '2026-09-30T09:00:00Z', 'replayed'))
    expect(readFrames(socket)).toHaveLength(0) // not before the replay is complete
    socket.receive({ type: 'history_complete' })
    expect(readFrames(socket)).toEqual([{ type: 'read', message_id: 'm1' }])
    socket.receive(broadcastFrame('m2', '2026-09-30T09:01:00Z', 'live'))
    socket.receive(broadcastFrame('m3', '2026-09-30T09:02:00Z', 'own', { sender_id: ME }))
    session.markRead() // own arrivals do not trigger it; an explicit call (page shown) does
    expect(readFrames(socket).map((frame) => frame.message_id)).toEqual(['m1', 'm2', 'm3'])
    session.markRead()
    expect(readFrames(socket)).toHaveLength(3)
    session.stop()
    // Closing the chat drops its timeline: the next open replays from the server.
    expect(selectTimeline(CHAT_ID)(stores.message.getState()).messages).toHaveLength(0)
  })

  test('a hidden page advances nothing until it becomes visible', async () => {
    let visible = false
    const { session, sockets, online } = harness({ visible: () => visible })
    await online()
    sockets[0]!.receive(broadcastFrame('m1', '2026-09-30T09:00:00Z', 'while hidden'))
    expect(readFrames(sockets[0]!)).toHaveLength(0)
    visible = true
    session.markRead()
    expect(readFrames(sockets[0]!)).toEqual([{ type: 'read', message_id: 'm1' }])
    session.stop()
  })
})

/**
 * TG-1208: sends made while a chat is still opening. The walkthrough saw Enter silently
 * ignored right after opening a chat (the composer gated on `connection === 'online'`) and,
 * once online but mid-replay, the own message rendered ABOVE the replayed history. Both are
 * the same race: a send before the session's first `history_complete`. These tests drive
 * that race deterministically through the fake socket.
 */
import { describe, expect, test } from 'bun:test'
import { selectTimeline } from '@tg/core'
import { AUTH_OK, broadcastFrame, CHAT_ID, harness, ME, settle } from './chatSessionHarness'

const contents = (stores: ReturnType<typeof harness>['stores']) =>
  selectTimeline(CHAT_ID)(stores.message.getState()).messages.map((row) =>
    row.type === 'broadcast' ? `${row.content}:${row.delivery_state ?? 'server'}` : row.type,
  )

const messageFrames = (socket: { sentFrames(): Array<Record<string, unknown>> }) =>
  socket.sentFrames().filter((frame) => frame.type === 'message')

describe('sending while the chat opens (TG-1208)', () => {
  test('before auth_ok: the send is parked, shown as sending, and goes out after the replay', async () => {
    const { session, sockets, stores } = harness()
    session.start()
    await settle()
    expect(session.status()).toBe('connecting')
    expect(session.sendable()).toBeTrue()
    expect(session.sendMessage('early')).toBeTrue()
    expect(contents(stores)).toEqual(['early:sending'])
    const socket = sockets[0]!
    socket.open()
    socket.receive(AUTH_OK)
    expect(messageFrames(socket)).toHaveLength(0)
    socket.receive(broadcastFrame('h1', '2026-09-30T09:00:00Z', 'history'))
    socket.receive({ type: 'history_complete' })
    await settle()
    expect(messageFrames(socket).map((frame) => frame.content)).toEqual(['early'])
    expect(contents(stores)).toEqual(['history:server', 'early:sending'])
    session.stop()
  })

  test('online but mid-replay: the own row stays below every replayed row, then reconciles', async () => {
    const { session, sockets, stores } = harness()
    session.start()
    await settle()
    const socket = sockets[0]!
    socket.open()
    socket.receive(AUTH_OK)
    socket.receive(broadcastFrame('h1', '2026-09-30T09:00:00Z', 'first'))
    expect(session.sendMessage('mine')).toBeTrue()
    expect(session.sendMessage('mine too')).toBeTrue()
    socket.receive(broadcastFrame('h2', '2026-09-30T09:01:00Z', 'second'))
    socket.receive({ type: 'history_complete' })
    await settle()
    expect(contents(stores)).toEqual(['first:server', 'second:server', 'mine:sending', 'mine too:sending'])
    const [first, second] = messageFrames(socket)
    expect([first?.content, second?.content]).toEqual(['mine', 'mine too'])
    socket.receive(
      broadcastFrame('m1', '2026-09-30T09:02:00Z', 'mine', {
        sender_id: ME,
        sender: 'me',
        client_message_id: first?.client_message_id,
      }),
    )
    expect(contents(stores)).toEqual(['first:server', 'second:server', 'mine:sent', 'mine too:sending'])
    session.stop()
  })

  test('auth failure while opening fails the parked rows and closes the gate', async () => {
    const { session, sockets, stores } = harness()
    session.start()
    await settle()
    expect(session.sendMessage('never')).toBeTrue()
    const seen: boolean[] = []
    session.onSendable((value) => seen.push(value))
    sockets[0]!.open()
    sockets[0]!.receive({ type: 'auth_fail', reason: 'nope' })
    expect(contents(stores)).toEqual(['never:failed'])
    expect(session.sendable()).toBeFalse()
    expect(seen.at(-1)).toBeFalse()
    expect(messageFrames(sockets[0]!)).toHaveLength(0)
    session.stop()
  })

  test('a parked send does not go out twice on the reconnect replay', async () => {
    const { session, sockets, clock } = harness()
    session.start()
    await settle()
    session.sendMessage('once')
    sockets[0]!.open()
    sockets[0]!.receive(AUTH_OK)
    sockets[0]!.receive({ type: 'history_complete' })
    await settle()
    sockets[0]!.dropFromServer()
    clock.advance(500)
    sockets[1]!.open()
    sockets[1]!.receive(AUTH_OK)
    sockets[1]!.receive({ type: 'history_complete' })
    await settle()
    expect(messageFrames(sockets[0]!)).toHaveLength(1)
    expect(messageFrames(sockets[1]!)).toHaveLength(0)
    expect(session.sendable()).toBeTrue()
    session.stop()
  })
})

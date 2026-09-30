// TG-011: codec fixtures are the EXACT serialised strings pinned server-side by
// tests/ws_frame_snapshot_legacy_test.rs and tests/ws_frame_snapshot_extension_test.rs —
// if those move, this file must move with them (and vice versa: neither may move alone,
// the protocol is frozen).
import { describe, expect, test } from 'bun:test'
import type { ServerFrame } from '../types'
import { encodeClientFrame, normalizeTypingAction, parseServerFrame } from './frameCodec'

const TYPING_EXACT =
  '{"type":"typing","content":"dra","action":"recording_voice","user_id":"00000000-0000-0000-0000-000000000003","username":"alice"}'

const AUTH_OK_EXACT =
  '{"type":"auth_ok","room_name":"general","members":[],"participants":[],"read_receipts":[],"statuses":[{"user_id":"00000000-0000-0000-0000-000000000003","status":{"kind":"online"}},{"user_id":"00000000-0000-0000-0000-000000000004","status":{"kind":"empty"}}]}'

const USER_STATUS_EXACT =
  '{"type":"user_status","user_id":"00000000-0000-0000-0000-000000000003","status":{"kind":"offline","last_seen":"2026-09-30T12:00:00Z"}}'

const MINIMAL_BROADCAST_EXACT =
  '{"type":"broadcast","message_id":"00000000-0000-0000-0000-00000000000a","sender_id":null,"sender":"alice","sender_avatar":"","content":"hello","attachment":null,"reply_to":null,"recalled_at":null,"edited_at":null,"timestamp":"2026-09-30T12:00:00Z","favorite_id":null,"forwarded_from":null,"reactions":[]}'

describe('server frame parsing', () => {
  test('parses the pinned typing frame with its granular action', () => {
    const frame = parseServerFrame(TYPING_EXACT)
    expect(frame).toEqual({
      type: 'typing',
      content: 'dra',
      action: 'recording_voice',
      user_id: '00000000-0000-0000-0000-000000000003',
      username: 'alice',
    })
  })

  test('degrades a missing or unknown typing action to plain typing (TG-007 rule)', () => {
    expect(parseServerFrame('{"type":"typing","content":"dra"}')).toMatchObject({ action: 'typing' })
    expect(parseServerFrame('{"type":"typing","content":"","action":"levitating"}')).toMatchObject({
      action: 'typing',
    })
    expect(normalizeTypingAction('cancel')).toBe('cancel')
    expect(normalizeTypingAction(42)).toBe('typing')
  })

  test('parses the pinned auth_ok with room_name and per-user statuses', () => {
    const frame = parseServerFrame(AUTH_OK_EXACT) as Extract<ServerFrame, { type: 'auth_ok' }>
    expect(frame.room_name).toBe('general')
    expect(frame.statuses).toEqual([
      { user_id: '00000000-0000-0000-0000-000000000003', status: { kind: 'online' } },
      { user_id: '00000000-0000-0000-0000-000000000004', status: { kind: 'empty' } },
    ])
  })

  test('parses the pinned user_status offline tier with last_seen', () => {
    expect(parseServerFrame(USER_STATUS_EXACT)).toEqual({
      type: 'user_status',
      user_id: '00000000-0000-0000-0000-000000000003',
      status: { kind: 'offline', last_seen: '2026-09-30T12:00:00Z' },
    })
  })

  test('parses the pinned minimal broadcast: nulls present, client_message_id absent', () => {
    const frame = parseServerFrame(MINIMAL_BROADCAST_EXACT) as Extract<ServerFrame, { type: 'broadcast' }>
    expect(frame.sender_id).toBeNull()
    expect(frame.recalled_at).toBeNull()
    expect('client_message_id' in frame).toBe(false)
    expect(frame.reactions).toEqual([])
  })

  test('accepts every skeleton frame type', () => {
    for (const raw of [
      '{"type":"chat_updated","chat":{"id":"c"}}',
      '{"type":"member_updated","member":{"user_id":"u"}}',
      '{"type":"topic_updated","topic":{"id":"t","title":"x","icon_emoji":"","closed":false,"pinned":true}}',
      '{"type":"message_views_updated","views":[{"message_id":"m","views":7}]}',
      '{"type":"poll_updated","message_id":"m","poll":{"id":"p","question":"q","closed":false,"total_voters":0,"options":[]}}',
      '{"type":"draft_updated","user_id":"u","text":"t","reply_to_message_id":null,"topic_id":null,"updated_at":"2026-09-30T12:00:00Z"}',
    ]) {
      expect(parseServerFrame(raw)).not.toBeNull()
    }
  })

  test('ignores unknown frame types and unparseable text instead of failing', () => {
    expect(parseServerFrame('{"type":"levitation_started","x":1}')).toBeNull()
    expect(parseServerFrame('not json')).toBeNull()
    expect(parseServerFrame('42')).toBeNull()
    expect(parseServerFrame('{"content":"no type"}')).toBeNull()
  })
})

describe('client frame encoding', () => {
  test('encodes the handshake and message frames the server deserialises', () => {
    expect(encodeClientFrame({ type: 'join', token: 'tok' })).toBe('{"type":"join","token":"tok"}')
    expect(encodeClientFrame({ type: 'auth', token: 'tok', password: 'pw' })).toBe(
      '{"type":"auth","token":"tok","password":"pw"}',
    )
    expect(encodeClientFrame({ type: 'message', content: 'hi', reply_to: 'm1', client_message_id: 'c1' })).toBe(
      '{"type":"message","content":"hi","reply_to":"m1","client_message_id":"c1"}',
    )
    expect(encodeClientFrame({ type: 'typing', content: '', action: 'cancel' })).toBe(
      '{"type":"typing","content":"","action":"cancel"}',
    )
    expect(encodeClientFrame({ type: 'reaction', message_id: 'm', emoji: '👍', active: true })).toBe(
      '{"type":"reaction","message_id":"m","emoji":"👍","active":true}',
    )
  })
})

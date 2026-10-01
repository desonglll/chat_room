import { describe, expect, test } from 'bun:test'
import type { BroadcastMessage, DisplayMessage, SystemMessage, UploadMessage } from './messageView'
import type { MessageLayoutEntry } from './messageViewportLayout'
import { computeMessageLayout, createMessageLayoutCache, localDayKey, messageKey } from './messageViewportLayout'

const BASE = Date.parse('2026-09-30T10:00:00Z')
const MINUTE = 60_000
// UTC days: deterministic regardless of the machine's zone.
const utcDay = (ms: number) => Math.floor(ms / 86_400_000)

function msg(id: string, sender: string, offsetMin: number, extra: Partial<BroadcastMessage> = {}): BroadcastMessage {
  return {
    type: 'broadcast',
    message_id: id,
    sender_id: sender,
    sender,
    sender_avatar: '',
    content: id,
    attachment: null,
    reply_to: null,
    recalled_at: null,
    edited_at: null,
    timestamp: new Date(BASE + offsetMin * MINUTE).toISOString(),
    favorite_id: null,
    forwarded_from: null,
    reactions: [],
    ...extra,
  }
}

const system = (key: string): SystemMessage => ({ type: 'system', key, content: 'joined' })

const layout = (messages: DisplayMessage[], showGroupIdentity = true) =>
  computeMessageLayout(messages, { currentUserId: 'me', showGroupIdentity, dayKeyOf: utcDay })

describe('computeMessageLayout grouping', () => {
  test('consecutive messages of one sender within the window form first/middle/last', () => {
    const entries = layout([msg('1', 'a', 0), msg('2', 'a', 1), msg('3', 'a', 2), msg('4', 'b', 3)])
    expect(entries.map((entry) => entry.groupPosition)).toEqual(['first', 'middle', 'last', 'single'])
  })

  test('a gap longer than the window splits the run', () => {
    const entries = layout([msg('1', 'a', 0), msg('2', 'a', 6), msg('3', 'a', 7)])
    expect(entries.map((entry) => entry.groupPosition)).toEqual(['single', 'first', 'last'])
  })

  test('a custom window is honoured', () => {
    const entries = computeMessageLayout([msg('1', 'a', 0), msg('2', 'a', 6)], {
      currentUserId: 'me',
      showGroupIdentity: true,
      groupWindowMs: 10 * MINUTE,
      dayKeyOf: utcDay,
    })
    expect(entries.map((entry) => entry.groupPosition)).toEqual(['first', 'last'])
  })

  test('a system row breaks a run and is itself single', () => {
    const entries = layout([msg('1', 'a', 0), system('s'), msg('2', 'a', 1)])
    expect(entries.map((entry) => entry.groupPosition)).toEqual(['single', 'single', 'single'])
    expect(entries[1]?.isOutgoing).toBe(false)
  })

  test('a day boundary breaks a run and starts a day', () => {
    // 10:00Z + 14h = 00:00Z next day.
    const entries = layout([msg('1', 'a', 837), msg('2', 'a', 839), msg('3', 'a', 841)])
    expect(entries.map((entry) => entry.startsDay)).toEqual([true, false, true])
    expect(entries.map((entry) => entry.groupPosition)).toEqual(['first', 'last', 'single'])
  })

  test('system rows without a timestamp inherit the running day', () => {
    const entries = layout([msg('1', 'a', 0), system('s'), msg('2', 'b', 1)])
    expect(entries.map((entry) => entry.startsDay)).toEqual([true, false, false])
    expect(entries[1]?.day).toBe(entries[0]?.day as number)
  })

  test('uploads group with the current user own messages', () => {
    const upload: UploadMessage = {
      type: 'upload',
      key: 'u1',
      room_id: 'c',
      file_name: 'a.png',
      mime_type: 'image/png',
      size_bytes: 1,
      preview_url: '',
      is_sensitive: false,
      content: '',
      phase: 'uploading',
      processed_bytes: 0,
      total_bytes: 1,
      status: 'pending',
      error: '',
      timestamp: new Date(BASE + MINUTE).toISOString(),
    }
    const entries = layout([msg('1', 'me', 0), upload])
    expect(entries.map((entry) => entry.groupPosition)).toEqual(['first', 'last'])
    expect(entries.every((entry) => entry.isOutgoing)).toBe(true)
  })
})

describe('computeMessageLayout identity', () => {
  test('group chats show the name on the first and the avatar on the last incoming row', () => {
    const entries = layout([msg('1', 'a', 0), msg('2', 'a', 1), msg('3', 'a', 2)])
    expect(entries.map((entry) => entry.showSenderName)).toEqual([true, false, false])
    expect(entries.map((entry) => entry.showAvatar)).toEqual([false, false, true])
  })

  test('outgoing rows never show identity', () => {
    const entries = layout([msg('1', 'me', 0), msg('2', 'me', 1)])
    expect(entries.map((entry) => entry.isOutgoing)).toEqual([true, true])
    expect(entries.some((entry) => entry.showAvatar || entry.showSenderName)).toBe(false)
  })

  test('private chats never show identity', () => {
    const entries = layout([msg('1', 'a', 0), msg('2', 'a', 1)], false)
    expect(entries.some((entry) => entry.showAvatar || entry.showSenderName)).toBe(false)
  })

  test('a null sender_id is incoming and groups by display name', () => {
    const entries = layout([msg('1', 'x', 0, { sender_id: null }), msg('2', 'x', 1, { sender_id: null })])
    expect(entries.map((entry) => entry.groupPosition)).toEqual(['first', 'last'])
    expect(entries[0]?.isOutgoing).toBe(false)
  })
})

describe('layout stability under prepend (the hidden-lead contract)', () => {
  test('rows after the lead keep their layout when older messages are prepended', () => {
    const tail = [msg('3', 'a', 10), msg('4', 'a', 11), msg('5', 'b', 12)]
    const withLead = layout([msg('2', 'a', 9), ...tail]).slice(1)
    const afterPrepend = layout([msg('0', 'c', 1), msg('1', 'a', 8), msg('2', 'a', 9), ...tail]).slice(3)
    expect(afterPrepend).toEqual(withLead)
  })
})

describe('keys and days', () => {
  test('messageKey prefers client_message_id so optimistic rows keep identity', () => {
    expect(messageKey(msg('pending:c1', 'me', 0, { client_message_id: 'c1' }))).toBe('c1')
    expect(messageKey(msg('m1', 'me', 0, { client_message_id: 'c1' }))).toBe('c1')
    expect(messageKey(msg('m2', 'a', 0))).toBe('m2')
    expect(messageKey(system('s1'))).toBe('s1')
  })

  test('localDayKey is monotonic across a day', () => {
    const start = localDayKey(BASE)
    expect(localDayKey(BASE + 60 * MINUTE)).toBeGreaterThanOrEqual(start)
    expect(localDayKey(BASE + 48 * 60 * MINUTE)).toBeGreaterThan(start)
  })

  test('an unparsable timestamp neither groups nor opens a day', () => {
    const entries = layout([msg('1', 'a', 0), msg('2', 'a', 1, { timestamp: 'garbage' })])
    expect(entries.map((entry) => entry.groupPosition)).toEqual(['single', 'single'])
    expect(entries[1]?.startsDay).toBe(false)
  })
})

describe('createMessageLayoutCache', () => {
  // Deterministic PRNG so a failure reproduces.
  function rng(seed: number) {
    let state = seed
    return () => {
      state = (state * 1_103_515_245 + 12_345) % 2_147_483_648
      return state / 2_147_483_648
    }
  }

  test('incremental layout equals a full layout after random appends, prepends, edits and inserts', () => {
    const random = rng(7)
    const cache = createMessageLayoutCache()
    const options = { currentUserId: 'me', showGroupIdentity: true, dayKeyOf: utcDay }
    let messages: DisplayMessage[] = []
    let head = 0
    let tail = 0
    const make = (minute: number) =>
      msg(`n${minute}-${random()}`, ['a', 'b', 'me'][Math.floor(random() * 3)] as string, minute)
    for (let step = 0; step < 400; step += 1) {
      const roll = random()
      if (roll < 0.35) {
        tail += Math.floor(random() * 8)
        messages = [...messages, make(tail)]
      } else if (roll < 0.55) {
        const page = Array.from({ length: 1 + Math.floor(random() * 5) }, () =>
          make((head -= Math.floor(random() * 700))),
        )
        messages = [...page.reverse(), ...messages]
      } else if (roll < 0.75 && messages.length > 0) {
        const at = Math.floor(random() * messages.length)
        messages = messages.map((message, index) => (index === at ? { ...message } : message))
      } else if (roll < 0.85 && messages.length > 0) {
        const at = Math.floor(random() * messages.length)
        messages = [...messages.slice(0, at), system(`s${step}`), ...messages.slice(at)]
      } else if (messages.length > 0) {
        const at = Math.floor(random() * messages.length)
        messages = messages.filter((_, index) => index !== at)
      }
      expect(cache(messages, options)).toEqual(computeMessageLayout(messages, options))
    }
  })

  test('unchanged rows keep object identity across an append', () => {
    const cache = createMessageLayoutCache()
    const options = { currentUserId: 'me', showGroupIdentity: true, dayKeyOf: utcDay }
    const first = [msg('1', 'a', 0), msg('2', 'b', 1), msg('3', 'a', 2)]
    const before = cache(first, options)
    const after = cache([...first, msg('4', 'a', 3)], options)
    expect(after[0]).toBe(before[0] as MessageLayoutEntry)
    expect(after[1]).toBe(before[1] as MessageLayoutEntry)
    expect(after[2]?.groupPosition).toBe('first')
  })

  test('changing options forces a full layout', () => {
    const cache = createMessageLayoutCache()
    const messages = [msg('1', 'a', 0), msg('2', 'a', 1)]
    cache(messages, { currentUserId: 'me', showGroupIdentity: true, dayKeyOf: utcDay })
    const privateLayout = cache(messages, { currentUserId: 'me', showGroupIdentity: false, dayKeyOf: utcDay })
    expect(privateLayout.some((entry) => entry.showSenderName)).toBe(false)
  })
})

describe('TG-904 channels', () => {
  test("every post is on the incoming side in a channel, the admin's own included", () => {
    const messages = [msg('1', 'me', 0), msg('2', 'me', 1), msg('3', 'other', 2)]
    const channel = computeMessageLayout(messages, {
      currentUserId: 'me',
      showGroupIdentity: false,
      channel: true,
      dayKeyOf: utcDay,
    })
    expect(channel.map((entry) => entry.isOutgoing)).toEqual([false, false, false])
    const group = computeMessageLayout(messages, { currentUserId: 'me', showGroupIdentity: false, dayKeyOf: utcDay })
    expect(group.map((entry) => entry.isOutgoing)).toEqual([true, true, false])
  })

  test('the cache recomputes when a chat becomes a channel', () => {
    const cache = createMessageLayoutCache()
    const messages = [msg('1', 'me', 0)]
    const base = { currentUserId: 'me', showGroupIdentity: false, dayKeyOf: utcDay }
    expect(cache(messages, base)[0]?.isOutgoing).toBe(true)
    expect(cache(messages, { ...base, channel: true })[0]?.isOutgoing).toBe(false)
  })
})

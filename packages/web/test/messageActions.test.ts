/**
 * TG-100: which bubble actions the chat binds, and what they do. Absent = hidden in the
 * bubble, so these are the permission rules the viewer sees.
 */
import { describe, expect, test } from 'bun:test'
import type { BroadcastMessage, ClientFrame, ComposerModeEvent, DisplayMessage } from '@tg/core'
import type { MessageActionDeps } from '../src/features/chat/messageActions'
import { bindMessageActions, canDeleteAll, deliveryFor } from '../src/features/chat/messageActions'

const ME = 'me'

function message(id: string, sender: string, extra: Partial<BroadcastMessage> = {}): BroadcastMessage {
  return {
    type: 'broadcast',
    message_id: id,
    sender_id: sender,
    sender,
    sender_avatar: '',
    content: 'hello',
    attachment: null,
    reply_to: null,
    recalled_at: null,
    edited_at: null,
    timestamp: '2026-10-01T10:00:00Z',
    favorite_id: null,
    forwarded_from: null,
    reactions: [],
    ...extra,
  }
}

function deps(overrides: Partial<MessageActionDeps> = {}) {
  const log: Array<[string, unknown]> = []
  const record =
    (name: string) =>
    (...args: unknown[]) => {
      log.push([name, args.length === 1 ? args[0] : args])
      return true
    }
  const value: MessageActionDeps = {
    chatId: 'c1',
    viewerId: ME,
    canPin: false,
    sendFrame: record('sendFrame') as (frame: ClientFrame) => boolean,
    dispatchMode: record('dispatchMode') as (chatId: string, event: ComposerModeEvent) => unknown,
    toggleSelected: record('toggleSelected'),
    openMedia: record('openMedia'),
    jumpTo: record('jumpTo'),
    requestDelete: record('requestDelete'),
    requestForward: record('requestForward'),
    pin: record('pin'),
    copy: record('copy'),
    ...overrides,
  }
  return { value, log }
}

describe('bindMessageActions', () => {
  test("others' messages: no edit, no delete; pin only with the permission", () => {
    const { value } = deps()
    const actions = bindMessageActions(message('m1', 'other'), value)
    expect(actions.onEdit).toBeUndefined()
    expect(actions.onDelete).toBeUndefined()
    expect(actions.onPin).toBeUndefined()
    expect(actions.onReply).toBeDefined()
    expect(actions.onForward).toBeDefined()
    expect(actions.onReact).toBeDefined()
    expect(bindMessageActions(message('m1', 'other'), deps({ canPin: true }).value).onPin).toBeDefined()
  })

  test('own messages: reply / edit open composer bars, delete and forward open dialogs', () => {
    const { value, log } = deps()
    const actions = bindMessageActions(message('m1', ME, { content: 'orig' }), value)
    actions.onReply?.()
    actions.onEdit?.()
    actions.onDelete?.()
    actions.onForward?.()
    actions.onSelect?.()
    actions.onCopy?.()
    actions.onOpenMedia?.('a1')
    actions.onJumpTo?.('m0')
    expect(log).toEqual([
      ['dispatchMode', ['c1', { type: 'reply', messageId: 'm1' }]],
      ['dispatchMode', ['c1', { type: 'edit', messageId: 'm1', text: 'orig' }]],
      ['requestDelete', ['m1']],
      ['requestForward', ['m1']],
      ['toggleSelected', 'm1'],
      ['copy', 'orig'],
      ['openMedia', 'a1'],
      ['jumpTo', 'm0'],
    ])
  })

  test('react toggles: active unless the viewer already chose that emoji', () => {
    const { value, log } = deps()
    const reacted = message('m1', 'other', { reactions: [{ emoji: '👍', user_ids: [ME] }] })
    const actions = bindMessageActions(reacted, value)
    actions.onReact?.('👍')
    actions.onReact?.('❤️')
    expect(log).toEqual([
      ['sendFrame', { type: 'reaction', message_id: 'm1', emoji: '👍', active: false }],
      ['sendFrame', { type: 'reaction', message_id: 'm1', emoji: '❤️', active: true }],
    ])
  })

  test('pending rows cannot be edited, deleted, forwarded or selected; captionless media cannot be edited', () => {
    const pending = bindMessageActions(message('pending:x', ME, { delivery_state: 'sending' }), deps().value)
    expect(pending.onEdit).toBeUndefined()
    expect(pending.onDelete).toBeUndefined()
    expect(pending.onForward).toBeUndefined()
    expect(pending.onSelect).toBeUndefined()
    const photo = bindMessageActions(message('m2', ME, { content: '' }), deps().value)
    expect(photo.onEdit).toBeUndefined()
    expect(photo.onCopy).toBeUndefined()
    expect(photo.onDelete).toBeDefined()
  })

  test('own polls cannot be edited (the server refuses it) but can still be deleted', () => {
    const poll = { id: 'm3', question: '午饭？', closed: false, total_voters: 0, options: [] }
    const actions = bindMessageActions(message('m3', ME, { content: '午饭？', poll }), deps().value)
    expect(actions.onEdit).toBeUndefined()
    expect(actions.onDelete).toBeDefined()
  })

  test('system rows get no actions', () => {
    const row: DisplayMessage = { type: 'system', key: 's', content: 'x joined' }
    expect(bindMessageActions(row, deps().value)).toEqual({})
  })
})

describe('deliveryFor / canDeleteAll', () => {
  test('outgoing acknowledged messages up to the peer cursor are read', () => {
    const own = message('m1', ME)
    expect(deliveryFor(own, ME, '')).toBeUndefined()
    expect(deliveryFor(own, ME, '2026-10-01T10:00:00Z')).toBe('read')
    expect(deliveryFor(own, ME, '2026-10-01T09:59:59Z')).toBeUndefined()
    expect(deliveryFor(message('m2', 'other'), ME, '2026-10-01T11:00:00Z')).toBeUndefined()
    expect(deliveryFor({ ...own, delivery_state: 'sending' }, ME, '2026-10-01T11:00:00Z')).toBeUndefined()
  })

  test('the selection bar deletes only when every selected message is own and live', () => {
    const rows = [message('a', ME), message('b', 'other'), message('c', ME, { recalled_at: 'x' })]
    expect(canDeleteAll(rows, new Set(['a']), ME)).toBeTrue()
    expect(canDeleteAll(rows, new Set(['a', 'b']), ME)).toBeFalse()
    expect(canDeleteAll(rows, new Set(['c']), ME)).toBeFalse()
    expect(canDeleteAll(rows, new Set(['missing']), ME)).toBeFalse()
    expect(canDeleteAll(rows, new Set(), ME)).toBeFalse()
  })
})

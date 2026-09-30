import { describe, expect, test } from 'bun:test'
import { buildMessageMenu, canReact } from '../messageMenu'
import { makeMessage } from '../fixtures/bubbleFixtures'
import type { MessageActions } from '../types'

const all: MessageActions = {
  onReply: () => {},
  onEdit: () => {},
  onDelete: () => {},
  onForward: () => {},
  onReact: () => {},
  onPin: () => {},
  onCopy: () => {},
  onSelect: () => {},
}
const ids = (message = makeMessage(), actions = all, delivered = true) =>
  buildMessageMenu(message, actions, { delivered }).map((item) => item.id)

describe('buildMessageMenu', () => {
  test('a delivered text message offers everything, delete last and separated', () => {
    const items = buildMessageMenu(makeMessage(), all, { delivered: true })
    expect(items.map((item) => item.id)).toEqual(['reply', 'edit', 'copy', 'pin', 'forward', 'select', 'delete'])
    expect(items.at(-1)).toMatchObject({ danger: true, separatorBefore: true })
  })

  test('an absent callback hides its row (permissions)', () => {
    expect(ids(makeMessage(), { onReply: all.onReply, onSelect: all.onSelect })).toEqual(['reply', 'select'])
    expect(ids(makeMessage(), {})).toEqual([])
  })

  test('a recalled message only offers selection', () => {
    expect(ids(makeMessage({ recalled_at: '2026-09-30T00:00:00Z' }))).toEqual(['select'])
  })

  test('an unacknowledged message cannot be replied to, edited, pinned or forwarded', () => {
    expect(ids(makeMessage(), all, false)).toEqual(['copy', 'select', 'delete'])
  })

  test('copy needs text', () => {
    expect(ids(makeMessage({ content: '  ' }))).not.toContain('copy')
  })

  test('canReact', () => {
    expect(canReact(makeMessage(), all, { delivered: true })).toBe(true)
    expect(canReact(makeMessage(), all, { delivered: false })).toBe(false)
    expect(canReact(makeMessage({ recalled_at: 'x' }), all, { delivered: true })).toBe(false)
    expect(canReact(makeMessage(), {}, { delivered: true })).toBe(false)
  })
})

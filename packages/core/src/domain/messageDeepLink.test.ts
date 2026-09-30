// Migrated from web/src/messageDeepLink.test.ts (TG-011). The route name is now a
// parameter defaulting to 'chat'; the last case pins that the old 'room' spelling still
// works when passed explicitly.
import { describe, expect, test } from 'bun:test'
import { messageIdFromRoute } from './messageDeepLink'

describe('message deep links', () => {
  test('returns a target only for the currently open chat', () => {
    expect(messageIdFromRoute('chat', 'chat-1', 'message-1', 'chat-1')).toBe('message-1')
    expect(messageIdFromRoute('chat', 'chat-2', 'message-1', 'chat-1')).toBe('')
    expect(messageIdFromRoute('favorites', 'chat-1', 'message-1', 'chat-1')).toBe('')
  })

  test('rejects array and empty query values', () => {
    expect(messageIdFromRoute('chat', 'chat-1', ['message-1'], 'chat-1')).toBe('')
    expect(messageIdFromRoute('chat', 'chat-1', '', 'chat-1')).toBe('')
  })

  test('supports the legacy route record name when asked to', () => {
    expect(messageIdFromRoute('room', 'chat-1', 'message-1', 'chat-1', 'room')).toBe('message-1')
    expect(messageIdFromRoute('room', 'chat-1', 'message-1', 'chat-1')).toBe('')
  })
})

import { describe, expect, test } from 'bun:test'
import { messageIdFromRoute } from './messageDeepLink'

describe('message deep links', () => {
  test('returns a target only for the currently open room', () => {
    expect(messageIdFromRoute('room', 'room-1', 'message-1', 'room-1')).toBe('message-1')
    expect(messageIdFromRoute('room', 'room-2', 'message-1', 'room-1')).toBe('')
    expect(messageIdFromRoute('favorites', 'room-1', 'message-1', 'room-1')).toBe('')
  })

  test('rejects array and empty query values', () => {
    expect(messageIdFromRoute('room', 'room-1', ['message-1'], 'room-1')).toBe('')
    expect(messageIdFromRoute('room', 'room-1', '', 'room-1')).toBe('')
  })
})

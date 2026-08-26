import { describe, expect, test } from 'bun:test'
import { aiContextUsage, aiSourceRoute } from './aiUi'

describe('AI context usage', () => {
  test('separates recent context from full-room RAG matches', () => {
    expect(aiContextUsage(130, 12)).toEqual({ recent: 118, retrieved: 12 })
  })

  test('does not claim RAG when no semantic evidence was injected', () => {
    expect(aiContextUsage(118, null)).toEqual({ recent: 118, retrieved: 0 })
  })

  test('links a RAG source to the original room message', () => {
    expect(
      aiSourceRoute({
        label: 'S1',
        room_id: 'room-1',
        message_id: 'message-9',
        sender: 'Ada',
        sent_at: '2026-08-25T10:00:00Z',
        excerpt: 'The launch date is Friday',
      }),
    ).toEqual({
      name: 'room',
      params: { id: 'room-1' },
      query: { message: 'message-9' },
    })
  })
})

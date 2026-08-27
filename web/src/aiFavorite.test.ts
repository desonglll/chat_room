import { describe, expect, test } from 'bun:test'
import { aiFavoriteContent, aiFavoriteTitle } from './aiFavorite'
import type { AiThreadMessage } from './types'

const message = {
  content: '决定周五发布 [S1, S2]',
  sources: [
    {
      label: 'S1',
      room_id: 'room one',
      message_id: 'message/1',
      sender: 'Ada',
      sent_at: '2026-08-27T10:00:00Z',
      excerpt: '周五发布',
    },
    {
      label: 'S2',
      room_id: 'room one',
      message_id: 'message/2',
      sender: 'Lin',
      sent_at: '2026-08-27T10:01:00Z',
      excerpt: '等待最终确认',
    },
  ],
} as AiThreadMessage

describe('AI summary favorites', () => {
  test('keeps the answer and cited source deep links', () => {
    const content = aiFavoriteContent(message)
    expect(aiFavoriteTitle('发布室')).toBe('发布室 · AI 回答')
    expect(content).toContain('决定周五发布 [S1, S2]')
    expect(content).toContain('/rooms/room%20one?message=message%2F1#message-message%2F1')
    expect(content).toContain('/rooms/room%20one?message=message%2F2#message-message%2F2')
  })
})

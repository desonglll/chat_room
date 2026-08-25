import { describe, expect, test } from 'bun:test'
import { pollAiThreadMessages } from './aiRunPolling'
import type { AiThreadMessage } from './types'

function assistant(status: AiThreadMessage['status'], content: string): AiThreadMessage {
  return {
    id: 'assistant-1',
    thread_id: 'thread-1',
    role: 'assistant',
    content,
    room_id: null,
    context_message_count: null,
    retrieved_message_count: null,
    status,
    revision: content.length,
    created_at: '2026-08-25T00:00:00Z',
    updated_at: '2026-08-25T00:00:00Z',
  }
}

describe('durable AI message polling', () => {
  test('keeps reading persisted revisions until the assistant message is terminal', async () => {
    const revisions = [assistant('pending', ''), assistant('streaming', '你'), assistant('completed', '你好')]
    const observed: string[] = []

    const completed = await pollAiThreadMessages(
      async () => [revisions.shift()!],
      (messages) => observed.push(messages[0].content),
      { intervalMs: 0 },
    )

    expect(observed).toEqual(['', '你', '你好'])
    expect(completed[0].status).toBe('completed')
  })
})

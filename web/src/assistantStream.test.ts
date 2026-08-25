import { describe, expect, test } from 'bun:test'
import { createSseParser } from './assistantApi'

describe('AI assistant SSE parser', () => {
  test('preserves JSON events split across network chunks', () => {
    const events: Array<{ event: string; data: string }> = []
    const parser = createSseParser((event) => events.push(event))

    parser.push('event: meta\ndata: {"room_id":"room-1","context_message_')
    parser.push('count":2,"context_format":"toon-v3-compatible"}\n\nevent: delta\ndata: {"content":"你')
    parser.push('好"}\n\nevent: done\ndata: {}\n\n')
    parser.finish()

    expect(events).toEqual([
      {
        event: 'meta',
        data: '{"room_id":"room-1","context_message_count":2,"context_format":"toon-v3-compatible"}',
      },
      { event: 'delta', data: '{"content":"你好"}' },
      { event: 'done', data: '{}' },
    ])
  })

  test('joins multiline data and ignores keep-alive comments', () => {
    const events: Array<{ event: string; data: string }> = []
    const parser = createSseParser((event) => events.push(event))

    parser.push(': keep-alive\r\n\r\nevent: message\r\ndata: first\r\ndata: second\r\n\r\n')
    parser.finish()

    expect(events).toEqual([{ event: 'message', data: 'first\nsecond' }])
  })
})

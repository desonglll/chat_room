import { describe, expect, test } from 'bun:test'
import { createSseParser } from './assistantApi'
import { streamAiThread } from './aiThreadApi'

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

  test('finishes on the done event without waiting for the transport to close', async () => {
    const originalFetch = globalThis.fetch
    const encoder = new TextEncoder()
    let reads = 0
    let canceled = false
    globalThis.fetch = async () =>
      ({
        ok: true,
        status: 200,
        body: {
          getReader: () => ({
            read: async () => {
              reads += 1
              if (reads > 1) throw new Error('read after done')
              return {
                done: false,
                value: encoder.encode(
                  'event: meta\ndata: {"thread_id":"thread-1","title":"hello","room_id":null,"context_message_count":0,"context_format":null}\n\n' +
                    'event: delta\ndata: {"content":"你好"}\n\n' +
                    'event: done\ndata: {}\n\n',
                ),
              }
            },
            cancel: async () => {
              canceled = true
            },
            releaseLock: () => {},
          }),
        },
      }) as Response

    try {
      const deltas: string[] = []
      const meta = await streamAiThread(
        'token',
        'thread-1',
        '你好',
        null,
        '',
        (delta) => deltas.push(delta),
        () => {},
      )

      expect(deltas).toEqual(['你好'])
      expect(meta.thread_id).toBe('thread-1')
      expect(reads).toBe(1)
      expect(canceled).toBeTrue()
    } finally {
      globalThis.fetch = originalFetch
    }
  })
})

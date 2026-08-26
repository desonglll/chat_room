import { afterEach, describe, expect, mock, test } from 'bun:test'
import { streamAiSuggestions } from './aiSuggestionApi'

const originalFetch = globalThis.fetch

afterEach(() => {
  globalThis.fetch = originalFetch
})

describe('streamAiSuggestions', () => {
  test('emits each complete NDJSON item without waiting for the whole response', async () => {
    const encoder = new TextEncoder()
    const chunks = [
      'event: chunk\ndata: "{\\"type\\":\\"suggestion\\",\\"content\\":\\"先确认时间\\"}\\n{\\"type\\":\\"sug',
      'gestion\\",\\"content\\":\\"我来整理\\"}\\n"\n\n',
    ]
    globalThis.fetch = mock(() =>
      Promise.resolve(
        new Response(
          new ReadableStream({
            start(controller) {
              for (const chunk of chunks) controller.enqueue(encoder.encode(chunk))
              controller.close()
            },
          }),
          { status: 200, headers: { 'Content-Type': 'text/event-stream' } },
        ),
      ),
    ) as typeof fetch

    const received: string[] = []
    await streamAiSuggestions('room-1', 'token', '', (item) => received.push(item.content))

    expect(received).toEqual(['先确认时间', '我来整理'])
  })
})

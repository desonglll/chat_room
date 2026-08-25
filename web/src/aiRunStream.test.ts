import { describe, expect, test } from 'bun:test'
import { readSseJsonStream } from './aiRunStream'

describe('AI run SSE parsing', () => {
  test('decodes revisions split across network chunks on one response', async () => {
    const encoder = new TextEncoder()
    const chunks = [
      'event: message\ndata: {"revision":1,"content":"你"}\n',
      '\nevent: message\r\ndata: {"revision":2,"content":"你好"}\r\n\r\n',
    ]
    const response = new Response(
      new ReadableStream({
        start(controller) {
          for (const chunk of chunks) controller.enqueue(encoder.encode(chunk))
          controller.close()
        },
      }),
    )
    const observed: Array<{ revision: number; content: string }> = []

    await readSseJsonStream(response, (message) => observed.push(message))

    expect(observed).toEqual([
      { revision: 1, content: '你' },
      { revision: 2, content: '你好' },
    ])
  })
})

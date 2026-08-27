import { afterEach, describe, expect, mock, test } from 'bun:test'
import { createAiRun } from './aiThreadApi'

const originalFetch = globalThis.fetch

afterEach(() => {
  globalThis.fetch = originalFetch
})

describe('AI run API', () => {
  test('sends selected message IDs without embedding message bodies', async () => {
    let body = ''
    globalThis.fetch = mock(async (_input: RequestInfo | URL, init?: RequestInit) => {
      body = String(init?.body)
      return new Response(
        JSON.stringify({
          id: 'run-1',
          thread_id: 'thread-1',
          user_message_id: 'user-message-1',
          assistant_message_id: 'assistant-message-1',
          client_request_id: 'request-1',
          room_id: 'room-1',
          purpose: 'question',
          source_after_message_id: null,
          source_through_message_id: null,
          source_message_count: null,
          model_option_id: null,
          provider: 'openai',
          model: 'test',
          status: 'queued',
          context_message_count: null,
          retrieved_message_count: null,
          error_message: null,
          created_at: '2026-08-27T10:00:00Z',
          updated_at: '2026-08-27T10:00:00Z',
        }),
        { status: 202, headers: { 'content-type': 'application/json' } },
      )
    }) as typeof fetch

    await createAiRun('token', 'thread-1', 'question', 'room-1', '', 'request-1', null, ['message-2', 'message-1'])

    expect(JSON.parse(body)).toEqual({
      question: 'question',
      room_id: 'room-1',
      client_request_id: 'request-1',
      model_option_id: null,
      message_ids: ['message-2', 'message-1'],
    })
    expect(body).not.toContain('private body')
  })
})

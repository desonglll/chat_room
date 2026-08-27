import { afterEach, describe, expect, mock, test } from 'bun:test'
import {
  AiExtractionApiError,
  createAiExtraction,
  getAiExtraction,
  updateAiExtractionCandidate,
  type AiExtractionCandidate,
} from './aiExtractionApi'

const originalFetch = globalThis.fetch

afterEach(() => {
  globalThis.fetch = originalFetch
})

describe('AI extraction API', () => {
  test('sends the selected range and room authorization without task state fields', async () => {
    const fetchMock = mock(async (_path: string, options?: RequestInit) =>
      Response.json({ id: 'run-1', status: 'queued', candidates: [], ...JSON.parse(String(options?.body)) }),
    )
    globalThis.fetch = fetchMock as typeof fetch
    await createAiExtraction(
      'room/one',
      'session-token',
      'room-secret',
      '2026-08-26T00:00:00.000Z',
      '2026-08-27T00:00:00.000Z',
      'model-1',
    )

    expect(String(fetchMock.mock.calls[0]![0])).toEndWith('/api/rooms/room%2Fone/ai/extractions')
    expect(fetchMock.mock.calls[0]![1]?.headers).toEqual({
      Accept: 'application/json',
      Authorization: 'Bearer session-token',
      'Content-Type': 'application/json',
      'x-room-password': 'room-secret',
    })
    const payload = JSON.parse(String(fetchMock.mock.calls[0]![1]?.body))
    expect(payload).toMatchObject({ model_option_id: 'model-1' })
    expect(payload).not.toHaveProperty('status')
    expect(payload).not.toHaveProperty('assignee_id')
  })

  test('polls and confirms candidates with encoded ids and optimistic versions', async () => {
    const fetchMock = mock(async () => Response.json({ id: 'candidate/one', version: 5 }))
    globalThis.fetch = fetchMock as typeof fetch
    await getAiExtraction('run/one', 'token', '')
    await updateAiExtractionCandidate(
      { id: 'candidate/one', version: 4 } as AiExtractionCandidate,
      'confirm',
      'token',
      'secret',
    )

    expect(String(fetchMock.mock.calls[0]![0])).toEndWith('/api/ai/extractions/run%2Fone')
    expect(String(fetchMock.mock.calls[1]![0])).toEndWith('/candidate%2Fone')
    expect(JSON.parse(String(fetchMock.mock.calls[1]![1]?.body))).toEqual({ action: 'confirm', version: 4 })
  })

  test('preserves candidate conflicts as typed errors', async () => {
    globalThis.fetch = mock(async () => new Response(null, { status: 409 })) as typeof fetch
    try {
      await updateAiExtractionCandidate(
        { id: 'candidate', version: 1 } as AiExtractionCandidate,
        'dismiss',
        'token',
        '',
      )
      throw new Error('expected conflict')
    } catch (error) {
      expect(error).toBeInstanceOf(AiExtractionApiError)
      expect((error as AiExtractionApiError).status).toBe(409)
    }
  })
})

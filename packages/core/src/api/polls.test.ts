// TG-406: the polls client against an injected fake fetch — paths, methods, bodies, query.
import { describe, expect, test } from 'bun:test'
import { ApiError, createApiClient } from './http'
import { closePoll, createPoll, getPoll, listPollVoters, retractPollVote, votePoll } from './polls'

function fakeFetch(status = 200, body: unknown = {}) {
  const calls: Array<{ url: string; init: RequestInit | undefined }> = []
  const client = createApiClient({
    fetchImpl: async (url, init) => {
      calls.push({ url, init })
      return Response.json(body, { status })
    },
  })
  return { calls, client }
}

const bodyOf = (init: RequestInit | undefined) => JSON.parse(String(init?.body)) as unknown

describe('polls api', () => {
  test('creates a poll in a chat with the wire field names', async () => {
    const { calls, client } = fakeFetch(201, { id: 'm1' })
    await createPoll(client, 't', 'chat/1', {
      question: 'Q',
      options: ['a', 'b'],
      quiz: true,
      correct_option: 1,
      explanation: 'why',
    })
    expect(calls[0]!.url).toBe('/api/chats/chat%2F1/polls')
    expect(calls[0]!.init?.method).toBe('POST')
    expect(bodyOf(calls[0]!.init)).toEqual({
      question: 'Q',
      options: ['a', 'b'],
      quiz: true,
      correct_option: 1,
      explanation: 'why',
    })
  })

  test('vote, retract, close and read address the poll by its message id', async () => {
    const { calls, client } = fakeFetch()
    await votePoll(client, 't', 'm1', [0, 2])
    await retractPollVote(client, 't', 'm1')
    await closePoll(client, 't', 'm1')
    await getPoll(client, 't', 'm1')
    expect(calls.map((call) => `${call.init?.method} ${call.url}`)).toEqual([
      'POST /api/polls/m1/votes',
      'DELETE /api/polls/m1/votes',
      'POST /api/polls/m1/close',
      'GET /api/polls/m1',
    ])
    expect(bodyOf(calls[0]!.init)).toEqual({ options: [0, 2] })
  })

  test('the voter list pages one option', async () => {
    const { calls, client } = fakeFetch(200, { option: 1, total: 0, voters: [] })
    await listPollVoters(client, 't', 'm1', 1, { limit: 20, offset: 40 })
    expect(calls[0]!.url).toBe('/api/polls/m1/voters?option=1&limit=20&offset=40')
  })

  test('a refused anonymous voter list surfaces as ApiError 403', async () => {
    const { client } = fakeFetch(403, {})
    const error = await listPollVoters(client, 't', 'm1', 0).catch((caught: unknown) => caught)
    expect(error).toBeInstanceOf(ApiError)
    expect((error as ApiError).status).toBe(403)
  })
})

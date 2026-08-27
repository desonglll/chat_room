import { afterEach, describe, expect, mock, test } from 'bun:test'
import { getRoomAiPolicy, updateRoomAiPolicy } from './roomAiPolicyApi'

const originalFetch = globalThis.fetch

afterEach(() => {
  globalThis.fetch = originalFetch
})

describe('Room AI policy API', () => {
  test('reads and updates an encoded Room with an optimistic version', async () => {
    const fetchMock = mock(async (_path: string, options?: RequestInit) =>
      Response.json({ room_id: 'room/one', mode: 'admins', version: 4, applies_to: 'new_runs_only' }),
    )
    globalThis.fetch = fetchMock as typeof fetch

    await getRoomAiPolicy('room/one', 'session-token')
    await updateRoomAiPolicy('room/one', 'session-token', 'admins', 3)

    expect(String(fetchMock.mock.calls[0]![0])).toEndWith('/api/rooms/room%2Fone/ai-policy')
    expect(fetchMock.mock.calls[0]![1]?.cache).toBe('no-store')
    expect(JSON.parse(String(fetchMock.mock.calls[1]![1]?.body))).toEqual({ mode: 'admins', version: 3 })
  })

  test('keeps policy conflicts actionable', async () => {
    globalThis.fetch = mock(async () => new Response(null, { status: 409 })) as typeof fetch
    expect(updateRoomAiPolicy('room', 'token', 'disabled', 1)).rejects.toThrow('其他窗口')
  })
})

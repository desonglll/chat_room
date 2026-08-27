import { afterEach, describe, expect, mock, test } from 'bun:test'
import { auditParams, listRoomAuditEvents, listSystemAuditEvents } from './auditApi'

const originalFetch = globalThis.fetch

afterEach(() => {
  globalThis.fetch = originalFetch
})

describe('audit API', () => {
  test('serializes actor, type, time range, cursor, and bounded page size', () => {
    const params = auditParams({
      actor: 'admin name',
      eventType: 'room.member.ban_requested',
      from: '2026-08-01T00:00:00.000Z',
      to: '2026-08-27T23:59:59.000Z',
      cursor: '2026-08-27T00:00:00Z|event-id',
      limit: 40,
    })
    expect(Object.fromEntries(params)).toEqual({
      actor: 'admin name',
      event_type: 'room.member.ban_requested',
      from: '2026-08-01T00:00:00.000Z',
      to: '2026-08-27T23:59:59.000Z',
      cursor: '2026-08-27T00:00:00Z|event-id',
      limit: '40',
    })
  })

  test('uses scoped endpoints and bearer authorization', async () => {
    const fetchMock = mock(async () => Response.json({ items: [], next_cursor: null }))
    globalThis.fetch = fetchMock as typeof fetch

    await listSystemAuditEvents('admin-token', { actor: 'alice' })
    await listRoomAuditEvents('room/with space', 'room-token', { eventType: 'room.member.remove_requested' })

    expect(String(fetchMock.mock.calls[0]![0])).toContain('/api/admin/audit-events?actor=alice')
    expect(fetchMock.mock.calls[0]![1]?.headers).toEqual({
      Accept: 'application/json',
      Authorization: 'Bearer admin-token',
    })
    expect(String(fetchMock.mock.calls[1]![0])).toContain('/api/rooms/room%2Fwith%20space/audit-events?')
  })

  test('surfaces permission failures instead of returning an empty page', async () => {
    globalThis.fetch = mock(async () => new Response(null, { status: 403 })) as typeof fetch
    expect(listSystemAuditEvents('member-token')).rejects.toThrow('没有读取审计记录的权限')
  })
})

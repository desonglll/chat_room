import { describe, expect, test } from 'bun:test'
import { globalSearchRouteQuery, readGlobalSearchFilters } from './useGlobalSearch'

describe('global search route state', () => {
  test('restores supported filters and normalizes unsafe route values', () => {
    expect(
      readGlobalSearchFilters({
        q: `  ${'x'.repeat(220)}  `,
        room: ['room-1', 'ignored'],
        sender: 'user-1',
        from: 'not-a-date',
        to: '2026-08-02',
        type: 'script',
      }),
    ).toEqual({
      q: 'x'.repeat(200),
      roomId: 'room-1',
      senderId: 'user-1',
      from: '',
      to: '2026-08-02',
      contentType: 'all',
    })
  })

  test('writes only query and filters, never result content or cursors', () => {
    const query = globalSearchRouteQuery({
      q: ' quarterly plan ',
      roomId: 'room-1',
      senderId: '',
      from: '2026-08-01',
      to: '',
      contentType: 'file',
    })
    expect(query).toEqual({ q: 'quarterly plan', room: 'room-1', from: '2026-08-01', type: 'file' })
    expect(query).not.toHaveProperty('items')
    expect(query).not.toHaveProperty('cursor')
  })
})

import { describe, expect, test } from 'bun:test'
import type { FavoriteItem } from './favorites'
import { savedMessagesTimeline, savedSourceLine } from './favorites'

function item(id: string, created_at: string, extra: Partial<FavoriteItem> = {}): FavoriteItem {
  return {
    id,
    owner_id: 'u1',
    owner_username: 'me',
    owner_display_name: 'Me',
    access: 'owner',
    version: 1,
    collaborator_count: 0,
    kind: 'manual',
    title: '',
    content: 'note',
    source_message_id: null,
    source_room_id: null,
    source_sender: '',
    source_room_name: '',
    attachment: null,
    created_at,
    updated_at: created_at,
    ...extra,
  }
}

describe('Saved Messages projection', () => {
  test('reads oldest first like a chat', () => {
    const ordered = savedMessagesTimeline([item('b', '2026-10-02T00:00:00Z'), item('a', '2026-10-01T00:00:00Z')])
    expect(ordered.map((entry) => entry.id)).toEqual(['a', 'b'])
  })

  test('a saved message names its source; a note has no header', () => {
    expect(savedSourceLine(item('n', 'x'))).toBe('')
    expect(
      savedSourceLine(item('m', 'x', { kind: 'message', source_sender: 'Alice', source_room_name: '项目组' })),
    ).toBe('来自 Alice · 项目组')
    expect(savedSourceLine(item('m', 'x', { kind: 'message', source_sender: 'Alice' }))).toBe('来自 Alice')
  })
})

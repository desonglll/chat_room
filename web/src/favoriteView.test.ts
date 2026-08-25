import { describe, expect, test } from 'bun:test'
import type { FavoriteItem } from './types'
import { favoriteKindLabel, matchesFavoriteFilter } from './favoriteView'

const base: FavoriteItem = {
  id: 'favorite-1',
  kind: 'message',
  title: '',
  content: 'hello',
  source_message_id: 'message-1',
  source_room_id: 'room-1',
  source_sender: 'Ada',
  source_room_name: 'Project',
  attachment: null,
  created_at: '2026-08-25T00:00:00Z',
  updated_at: '2026-08-25T00:00:00Z',
}

describe('favorite view categories', () => {
  test('groups every attachment under files regardless of its legacy kind', () => {
    const image = {
      ...base,
      attachment: {
        id: 'attachment-1',
        file_name: 'design.png',
        mime_type: 'image/png',
        size_bytes: 128,
        download_url: '/file',
        is_sensitive: false,
      },
    }
    expect(matchesFavoriteFilter(image, 'file')).toBe(true)
    expect(matchesFavoriteFilter(image, 'message')).toBe(false)
    expect(favoriteKindLabel(image)).toBe('文件')
  })

  test('keeps attachment-free messages and manual notes separate', () => {
    expect(matchesFavoriteFilter(base, 'message')).toBe(true)
    expect(matchesFavoriteFilter({ ...base, kind: 'manual' }, 'manual')).toBe(true)
  })
})

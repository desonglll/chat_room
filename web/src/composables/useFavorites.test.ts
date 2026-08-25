import { afterEach, describe, expect, test } from 'bun:test'
import { ref } from 'vue'
import type { FavoriteItem } from '../types'
import { useFavorites } from './useFavorites'

const originalFetch = globalThis.fetch
const favorite: FavoriteItem = {
  id: 'favorite-1',
  kind: 'message',
  title: '',
  content: 'hello',
  source_message_id: 'message-1',
  source_room_id: 'room-1',
  source_sender: 'alice',
  source_room_name: 'room',
  attachment: null,
  created_at: '2026-08-25T00:00:00Z',
  updated_at: '2026-08-25T00:00:00Z',
}

afterEach(() => {
  globalThis.fetch = originalFetch
})

describe('favorite message state', () => {
  test('removes and restores the active message id when toggled', async () => {
    const requests: Array<{ path: string; method: string }> = []
    globalThis.fetch = (async (input, init) => {
      requests.push({ path: String(input), method: init?.method || 'GET' })
      return init?.method === 'DELETE'
        ? new Response(null, { status: 204 })
        : new Response(JSON.stringify([favorite]), { status: 200 })
    }) as typeof fetch

    const favorites = useFavorites(ref(''))
    favorites.items.value = [favorite]
    expect(favorites.messageIds.value).toEqual(['message-1'])

    expect(await favorites.updateMessages(['message-1'])).toEqual({ active: false, count: 1 })
    expect(favorites.messageIds.value).toEqual([])
    expect(requests[0]).toEqual({ path: '/api/favorites/favorite-1', method: 'DELETE' })

    expect(await favorites.updateMessages(['message-1'])).toEqual({ active: true, count: 1 })
    expect(favorites.messageIds.value).toEqual(['message-1'])
    expect(requests[1]).toEqual({ path: '/api/favorites/messages', method: 'POST' })
  })
})

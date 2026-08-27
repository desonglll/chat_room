import { describe, expect, test } from 'bun:test'
import { resolveTarget } from './useAppPages'
import type { Room } from '../types'

const room: Room = {
  id: 'room-1',
  name: 'room',
  has_password: false,
  creator_user_id: 'user-1',
  join_policy: 'open',
  avatar_emoji: '',
  description: '',
  membership_status: 'active',
  membership_role: 'member',
  unread_count: 0,
  created_at: '2026-08-19T00:00:00Z',
}

describe('app page routes', () => {
  test('keeps an active member in the room while the socket reconnects', () => {
    expect(resolveTarget('chat', room, false)).toEqual({ name: 'room', params: { id: 'room-1' } })
  })

  test('uses the join route when durable membership is unavailable', () => {
    expect(resolveTarget('chat', { ...room, membership_status: 'pending' }, false)).toEqual({
      name: 'room-join',
      params: { id: 'room-1' },
    })
  })

  test('routes the contacts workspace independently from the selected room', () => {
    expect(resolveTarget('contacts', room, true)).toEqual({ name: 'contacts' })
  })

  test('routes the personal favorites independently from the selected room', () => {
    expect(resolveTarget('favorites', room, true)).toEqual({ name: 'favorites' })
  })

  test('routes global search independently from the selected room', () => {
    expect(resolveTarget('search', room, true)).toEqual({ name: 'search' })
  })

  test('routes the AI workspace independently from the selected room', () => {
    expect(resolveTarget('assistant', room, true)).toEqual({ name: 'assistant' })
  })
})

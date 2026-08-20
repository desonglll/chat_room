import { describe, expect, test } from 'bun:test'
import { contactEntries, contactSection, filterContactEntries } from './contactDirectory'
import type { FriendRequest, SocialUser } from './types'

const friend: SocialUser = {
  id: 'friend-1',
  username: 'shinoda',
  avatar_emoji: '',
  display_name: 'Shinoda',
  signature: 'Designing quietly',
  relationship: 'friend',
}
const incoming: FriendRequest = {
  user: { id: 'incoming-1', username: 'alice', avatar_emoji: '', display_name: 'Alice' },
  direction: 'incoming',
  created_at: '2026-08-20T00:00:00Z',
}
const outgoing: FriendRequest = {
  user: { id: 'outgoing-1', username: 'bob', avatar_emoji: '', display_name: 'Bob' },
  direction: 'outgoing',
  created_at: '2026-08-20T00:00:00Z',
}

describe('contact directory', () => {
  test('restores a valid section from the contacts URL', () => {
    expect(contactSection('requests')).toBe('requests')
    expect(contactSection('unknown')).toBe('friends')
  })

  test('builds stable entries for each contact section', () => {
    expect(contactEntries('friends', [friend], [incoming], [outgoing], [])).toMatchObject([
      { key: 'friend:friend-1', kind: 'friend', subtitle: 'Designing quietly' },
    ])
    expect(contactEntries('requests', [friend], [incoming], [outgoing], []).map((entry) => entry.kind)).toEqual([
      'incoming',
      'outgoing',
    ])
  })

  test('searches display names, usernames, and signatures case-insensitively', () => {
    const entries = contactEntries('friends', [friend], [], [], [])
    expect(filterContactEntries(entries, 'SHINODA')).toHaveLength(1)
    expect(filterContactEntries(entries, 'quietly')).toHaveLength(1)
    expect(filterContactEntries(entries, 'missing')).toEqual([])
  })
})

import { describe, expect, test } from 'bun:test'
import { clearBootstrapSnapshot, readBootstrapSnapshot, writeBootstrapSnapshot } from './bootstrapSnapshot'
import type { Room, User } from './types'

class MemoryStorage {
  private values = new Map<string, string>()

  getItem(key: string): string | null {
    return this.values.get(key) ?? null
  }

  setItem(key: string, value: string): void {
    this.values.set(key, value)
  }

  removeItem(key: string): void {
    this.values.delete(key)
  }
}

const user: User = {
  id: 'user-1',
  username: 'alice',
  avatar_emoji: '',
  display_name: 'Alice',
  signature: '',
  homepage: '',
  created_at: '2026-08-19T00:00:00Z',
}
const room: Room = {
  id: 'room-1',
  name: 'General',
  has_password: false,
  creator_user_id: user.id,
  join_policy: 'open',
  avatar_emoji: '',
  description: '',
  membership_status: 'active',
  membership_role: 'owner',
  unread_count: 0,
  created_at: '2026-08-19T00:00:00Z',
}

describe('bootstrap snapshot', () => {
  test('restores a current authenticated snapshot without credentials or messages', () => {
    const storage = new MemoryStorage()
    writeBootstrapSnapshot(storage, user, [room], 1_000)
    expect(readBootstrapSnapshot(storage, true, 2_000)).toEqual({
      version: 1,
      savedAt: 1_000,
      user,
      rooms: [room],
    })
  })

  test('rejects expired, anonymous, and explicitly cleared snapshots', () => {
    const storage = new MemoryStorage()
    writeBootstrapSnapshot(storage, user, [room], 1_000)
    expect(readBootstrapSnapshot(storage, false, 2_000)).toBeNull()
    expect(readBootstrapSnapshot(storage, true, 7 * 60 * 60 * 1_000)).toBeNull()
    clearBootstrapSnapshot(storage)
    expect(readBootstrapSnapshot(storage, true, 2_000)).toBeNull()
  })
})

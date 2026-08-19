import type { Room, User } from './types'

const SNAPSHOT_KEY = 'chat-room.bootstrap.v1'
const SNAPSHOT_VERSION = 1
const MAX_AGE_MS = 6 * 60 * 60 * 1000

export interface BootstrapSnapshot {
  version: 1
  savedAt: number
  user: User
  rooms: Room[]
}

interface StorageLike {
  getItem(key: string): string | null
  setItem(key: string, value: string): void
  removeItem(key: string): void
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}

function isUser(value: unknown): value is User {
  return isRecord(value) && typeof value.id === 'string' && typeof value.username === 'string'
}

function isRoom(value: unknown): value is Room {
  return isRecord(value) && typeof value.id === 'string' && typeof value.name === 'string'
}

export function readBootstrapSnapshot(
  storage: StorageLike,
  hasSession: boolean,
  now = Date.now(),
): BootstrapSnapshot | null {
  if (!hasSession) return null
  try {
    const parsed: unknown = JSON.parse(storage.getItem(SNAPSHOT_KEY) || 'null')
    if (
      !isRecord(parsed) ||
      parsed.version !== SNAPSHOT_VERSION ||
      typeof parsed.savedAt !== 'number' ||
      now - parsed.savedAt > MAX_AGE_MS ||
      now < parsed.savedAt ||
      !isUser(parsed.user) ||
      !Array.isArray(parsed.rooms) ||
      !parsed.rooms.every(isRoom)
    ) {
      return null
    }
    return parsed as unknown as BootstrapSnapshot
  } catch {
    return null
  }
}

export function writeBootstrapSnapshot(storage: StorageLike, user: User | null, rooms: Room[], now = Date.now()): void {
  if (!user) {
    clearBootstrapSnapshot(storage)
    return
  }
  try {
    storage.setItem(
      SNAPSHOT_KEY,
      JSON.stringify({ version: SNAPSHOT_VERSION, savedAt: now, user, rooms } satisfies BootstrapSnapshot),
    )
  } catch {
    // The live application does not depend on optional browser storage.
  }
}

export function clearBootstrapSnapshot(storage: StorageLike): void {
  try {
    storage.removeItem(SNAPSHOT_KEY)
  } catch {
    // Storage can be unavailable in privacy modes.
  }
}

import { afterEach, beforeEach, describe, expect, test } from 'bun:test'
import {
  clearRememberedRoomPasswords,
  clearRoomPassword,
  readRoomPassword,
  saveRoomPassword,
} from './roomPasswordVault'

class MemoryStorage implements Storage {
  private data = new Map<string, string>()
  get length(): number {
    return this.data.size
  }
  clear(): void {
    this.data.clear()
  }
  getItem(key: string): string | null {
    return this.data.get(key) ?? null
  }
  key(index: number): string | null {
    return [...this.data.keys()][index] ?? null
  }
  removeItem(key: string): void {
    this.data.delete(key)
  }
  setItem(key: string, value: string): void {
    this.data.set(key, value)
  }
}

const originalWindow = globalThis.window

beforeEach(() => {
  globalThis.window = {
    localStorage: new MemoryStorage(),
    sessionStorage: new MemoryStorage(),
  } as unknown as Window & typeof globalThis
})

afterEach(() => {
  if (originalWindow) globalThis.window = originalWindow
  else Reflect.deleteProperty(globalThis, 'window')
})

describe('room password vault', () => {
  test('persists verified passwords only when remembering is enabled', () => {
    saveRoomPassword('room-1', 'secret', true)
    expect(readRoomPassword('room-1', true)).toBe('secret')
    expect(readRoomPassword('room-1', false)).toBe('')

    saveRoomPassword('room-2', 'must-not-persist', false)
    expect(readRoomPassword('room-2', false)).toBe('')
    expect(readRoomPassword('room-2', true)).toBe('')
  })

  test('clears a room from both stores and can purge all remembered passwords', () => {
    saveRoomPassword('room-1', 'one', true)
    saveRoomPassword('room-2', 'two', true)
    clearRoomPassword('room-1')
    expect(readRoomPassword('room-1', true)).toBe('')
    clearRememberedRoomPasswords()
    expect(readRoomPassword('room-2', true)).toBe('')
  })
})

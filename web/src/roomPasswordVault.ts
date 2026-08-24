import { storageGet, storageSet } from './browserStorage'

const PREFIX = 'chat-room.password.'

function key(roomId: string): string {
  return `${PREFIX}${roomId}`
}

export function readRoomPassword(roomId: string, remember: boolean): string {
  return remember ? storageGet(window.localStorage, key(roomId)) : ''
}

export function saveRoomPassword(roomId: string, password: string, remember: boolean): void {
  if (remember) storageSet(window.localStorage, key(roomId), password)
}

export function clearRoomPassword(roomId: string): void {
  storageSet(window.localStorage, key(roomId), '')
  storageSet(window.sessionStorage, key(roomId), '')
}

export function clearRememberedRoomPasswords(): void {
  for (const storage of [window.localStorage, window.sessionStorage]) {
    try {
      const keys = Array.from({ length: storage.length }, (_, index) => storage.key(index))
      keys
        .filter((candidate): candidate is string => Boolean(candidate?.startsWith(PREFIX)))
        .forEach((candidate) => storage.removeItem(candidate))
    } catch {
      /* Browser storage may be disabled. */
    }
  }
}

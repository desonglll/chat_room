export function storageGet(storage: Storage, key: string): string {
  try {
    return storage.getItem(key) || ''
  } catch {
    return ''
  }
}

export function storageSet(storage: Storage, key: string, value: string): void {
  try {
    if (value) storage.setItem(key, value)
    else storage.removeItem(key)
  } catch {
    // Browser storage is optional; the active session remains usable without it.
  }
}

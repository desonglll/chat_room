/**
 * TG-606: the app-wide keyboard map, modelled on Telegram Desktop but avoiding keys the browser
 * keeps for itself (Ctrl+Tab, Ctrl+1…9, Ctrl+F). Pure, so the whole map is unit-tested.
 *
 *   Ctrl/⌘+K, or / outside a text field   search chats
 *   Alt+↑ / Alt+↓                         previous / next chat in the list
 *   Ctrl/⌘+Shift+1…9                      chat folder 1…9 (1 = «全部»)
 *   Ctrl/⌘+,                              settings
 *   Ctrl/⌘+↑ / Ctrl/⌘+↓ (in the composer) reply to the previous / next message
 *   Esc                                   close the open layer (handled by each layer)
 */

export type ShortcutAction =
  | { type: 'search' }
  | { type: 'chat'; step: 1 | -1 }
  | { type: 'folder'; index: number }
  | { type: 'settings' }

export interface KeyInput {
  key: string
  code?: string
  ctrlKey: boolean
  metaKey: boolean
  altKey: boolean
  shiftKey: boolean
  /** Focus is in a text field (input, textarea, contenteditable). */
  editable: boolean
}

export function globalShortcut(input: KeyInput): ShortcutAction | null {
  const command = input.ctrlKey || input.metaKey
  if (command && !input.altKey && !input.shiftKey && input.key.toLowerCase() === 'k') return { type: 'search' }
  if (!command && !input.altKey && !input.shiftKey && input.key === '/' && !input.editable) return { type: 'search' }
  if (input.altKey && !command && !input.shiftKey) {
    if (input.key === 'ArrowUp') return { type: 'chat', step: -1 }
    if (input.key === 'ArrowDown') return { type: 'chat', step: 1 }
  }
  if (command && input.shiftKey && !input.altKey) {
    // Shift turns the digit into a symbol on most layouts, so read the physical key.
    const digit = /^Digit([1-9])$/.exec(input.code ?? '')?.[1]
    if (digit) return { type: 'folder', index: Number(digit) }
  }
  if (command && !input.altKey && !input.shiftKey && input.key === ',') return { type: 'settings' }
  return null
}

/** Ctrl/⌘+↑/↓ in the composer: −1 = an older message, +1 = a newer one. */
export function replyStep(input: KeyInput): -1 | 1 | null {
  if (!(input.ctrlKey || input.metaKey) || input.altKey || input.shiftKey) return null
  if (input.key === 'ArrowUp') return -1
  if (input.key === 'ArrowDown') return 1
  return null
}

/**
 * The next reply target walking `ids` (oldest first) from `current`. Up from nothing starts at
 * the newest; down past the newest clears the reply (`null`), like Telegram Desktop.
 */
export function nextReplyTarget(ids: readonly string[], current: string | null, step: -1 | 1): string | null {
  if (ids.length === 0) return null
  const at = current === null ? ids.length : ids.indexOf(current)
  if (at < 0) return step === -1 ? (ids[ids.length - 1] ?? null) : null
  const next = at + step
  if (next >= ids.length) return null
  return ids[Math.max(0, next)] ?? null
}

/** The neighbour of `current` in an ordered list of chat ids (wrapping at the ends). */
export function neighbourChat(ids: readonly string[], current: string, step: 1 | -1): string | null {
  if (ids.length === 0) return null
  const at = ids.indexOf(current)
  if (at < 0) return ids[step === 1 ? 0 : ids.length - 1] ?? null
  return ids[(at + step + ids.length) % ids.length] ?? null
}

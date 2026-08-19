import type { PrivacyLockShortcut } from './types'

export const DEFAULT_PRIVACY_LOCK_SHORTCUT: PrivacyLockShortcut = {
  code: 'KeyL',
  primary: true,
  alt: false,
  shift: true,
}

interface LockKeyEvent {
  code: string
  repeat: boolean
  metaKey: boolean
  ctrlKey: boolean
  altKey: boolean
  shiftKey: boolean
}

const MODIFIER_CODES = new Set([
  'AltLeft',
  'AltRight',
  'ControlLeft',
  'ControlRight',
  'MetaLeft',
  'MetaRight',
  'ShiftLeft',
  'ShiftRight',
])

const KEY_LABELS: Record<string, string> = {
  Backquote: '`',
  Backslash: '\\',
  BracketLeft: '[',
  BracketRight: ']',
  Comma: ',',
  Equal: '=',
  Minus: '-',
  Period: '.',
  Quote: "'",
  Semicolon: ';',
  Slash: '/',
  Space: 'Space',
}

function cloneDefault(): PrivacyLockShortcut {
  return { ...DEFAULT_PRIVACY_LOCK_SHORTCUT }
}

export function isValidPrivacyLockShortcut(value: unknown): value is PrivacyLockShortcut {
  if (!value || typeof value !== 'object') return false
  const shortcut = value as Partial<PrivacyLockShortcut>
  return (
    typeof shortcut.code === 'string' &&
    shortcut.code.length > 0 &&
    shortcut.code.length <= 32 &&
    typeof shortcut.primary === 'boolean' &&
    typeof shortcut.alt === 'boolean' &&
    typeof shortcut.shift === 'boolean' &&
    (shortcut.primary || shortcut.alt) &&
    !MODIFIER_CODES.has(shortcut.code)
  )
}

export function parsePrivacyLockShortcut(value: string): PrivacyLockShortcut {
  if (!value) return cloneDefault()
  try {
    const parsed: unknown = JSON.parse(value)
    return isValidPrivacyLockShortcut(parsed) ? parsed : cloneDefault()
  } catch {
    return cloneDefault()
  }
}

export function privacyLockShortcutFromEvent(
  event: LockKeyEvent,
  apple = isApplePlatform(),
): PrivacyLockShortcut | null {
  if (event.repeat || !event.code || MODIFIER_CODES.has(event.code)) return null
  const unsupportedSystemModifier = apple ? event.ctrlKey : event.metaKey
  if (unsupportedSystemModifier) return null
  const shortcut: PrivacyLockShortcut = {
    code: event.code,
    primary: apple ? event.metaKey : event.ctrlKey,
    alt: event.altKey,
    shift: event.shiftKey,
  }
  return isValidPrivacyLockShortcut(shortcut) ? shortcut : null
}

export function matchesPrivacyLockShortcut(
  event: LockKeyEvent,
  shortcut: PrivacyLockShortcut,
  apple = isApplePlatform(),
): boolean {
  const unsupportedSystemModifier = apple ? event.ctrlKey : event.metaKey
  return (
    !event.repeat &&
    !unsupportedSystemModifier &&
    event.code === shortcut.code &&
    (apple ? event.metaKey : event.ctrlKey) === shortcut.primary &&
    event.altKey === shortcut.alt &&
    event.shiftKey === shortcut.shift
  )
}

export function isApplePlatform(platform?: string): boolean {
  const value = platform ?? (typeof navigator === 'undefined' ? '' : navigator.platform)
  return /Mac|iPhone|iPad/i.test(value)
}

function keyLabel(code: string): string {
  if (KEY_LABELS[code]) return KEY_LABELS[code]
  if (code.startsWith('Key')) return code.slice(3)
  if (code.startsWith('Digit')) return code.slice(5)
  return code.replace(/(Left|Right)$/, '')
}

export function formatPrivacyLockShortcut(shortcut: PrivacyLockShortcut, apple = isApplePlatform()): string {
  const parts: string[] = []
  if (shortcut.primary) parts.push(apple ? '⌘' : 'Ctrl')
  if (shortcut.alt) parts.push(apple ? '⌥' : 'Alt')
  if (shortcut.shift) parts.push(apple ? '⇧' : 'Shift')
  parts.push(keyLabel(shortcut.code))
  return parts.join(apple ? '' : ' + ')
}

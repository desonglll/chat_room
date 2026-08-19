import { describe, expect, test } from 'bun:test'
import {
  DEFAULT_PRIVACY_LOCK_SHORTCUT,
  formatPrivacyLockShortcut,
  matchesPrivacyLockShortcut,
  parsePrivacyLockShortcut,
  privacyLockShortcutFromEvent,
} from './privacyLock'

const event = (overrides: Partial<KeyboardEvent> = {}) =>
  ({
    code: 'KeyL',
    repeat: false,
    metaKey: false,
    ctrlKey: true,
    altKey: false,
    shiftKey: true,
    ...overrides,
  }) as KeyboardEvent

describe('privacy lock shortcuts', () => {
  test('uses the platform primary modifier', () => {
    expect(matchesPrivacyLockShortcut(event(), DEFAULT_PRIVACY_LOCK_SHORTCUT, false)).toBe(true)
    expect(
      matchesPrivacyLockShortcut(event({ ctrlKey: false, metaKey: true }), DEFAULT_PRIVACY_LOCK_SHORTCUT, true),
    ).toBe(true)
    expect(
      matchesPrivacyLockShortcut(event({ ctrlKey: true, metaKey: false }), DEFAULT_PRIVACY_LOCK_SHORTCUT, true),
    ).toBe(false)
    expect(matchesPrivacyLockShortcut(event({ shiftKey: false }), DEFAULT_PRIVACY_LOCK_SHORTCUT, false)).toBe(false)
  })

  test('records only modified non-modifier keys', () => {
    expect(privacyLockShortcutFromEvent(event({ code: 'KeyK', shiftKey: false }), false)).toEqual({
      code: 'KeyK',
      primary: true,
      alt: false,
      shift: false,
    })
    expect(privacyLockShortcutFromEvent(event({ ctrlKey: false, shiftKey: false }), false)).toBeNull()
    expect(privacyLockShortcutFromEvent(event({ code: 'ShiftLeft' }), false)).toBeNull()
  })

  test('formats macOS and Windows labels and safely restores stored values', () => {
    expect(formatPrivacyLockShortcut(DEFAULT_PRIVACY_LOCK_SHORTCUT, true)).toBe('⌘⇧L')
    expect(formatPrivacyLockShortcut(DEFAULT_PRIVACY_LOCK_SHORTCUT, false)).toBe('Ctrl + Shift + L')
    expect(parsePrivacyLockShortcut('{broken')).toEqual(DEFAULT_PRIVACY_LOCK_SHORTCUT)
    expect(parsePrivacyLockShortcut(JSON.stringify({ code: 'KeyP', primary: false, alt: true, shift: false }))).toEqual(
      { code: 'KeyP', primary: false, alt: true, shift: false },
    )
  })
})

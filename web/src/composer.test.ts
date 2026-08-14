import { describe, expect, test } from 'bun:test'
import { shouldSubmitMessage, type ComposerKeyEvent } from './composer'

function keyEvent(overrides: Partial<ComposerKeyEvent> = {}): ComposerKeyEvent {
  return {
    key: 'Enter',
    shiftKey: false,
    isComposing: false,
    keyCode: 13,
    ...overrides,
  }
}

describe('message composer Enter handling', () => {
  test('submits a normal Enter', () => {
    expect(shouldSubmitMessage(keyEvent(), false)).toBe(true)
  })

  test('does not submit while an IME composition is active', () => {
    expect(shouldSubmitMessage(keyEvent({ isComposing: true }), false)).toBe(false)
    expect(shouldSubmitMessage(keyEvent(), true)).toBe(false)
    expect(shouldSubmitMessage(keyEvent(), false, true)).toBe(false)
    expect(shouldSubmitMessage(keyEvent({ keyCode: 229 }), false)).toBe(false)
  })

  test('keeps Shift+Enter and non-Enter keys in the editor', () => {
    expect(shouldSubmitMessage(keyEvent({ shiftKey: true }), false)).toBe(false)
    expect(shouldSubmitMessage(keyEvent({ key: 'a', keyCode: 65 }), false)).toBe(false)
  })
})

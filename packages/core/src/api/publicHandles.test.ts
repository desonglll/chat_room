import { describe, expect, test } from 'bun:test'
import { publicChatPath, usernameReasonText } from './publicHandles'

describe('public handle copy', () => {
  test('every server reason has Chinese copy, unknown ones a fallback', () => {
    for (const reason of [
      'too_short',
      'too_long',
      'invalid_characters',
      'must_start_with_letter',
      'invalid_underscores',
      'reserved',
      'taken',
      'password_protected',
    ]) {
      expect(usernameReasonText(reason)).not.toBe('无法使用这个名称')
    }
    expect(usernameReasonText('something_new')).toBe('无法使用这个名称')
    expect(usernameReasonText(undefined)).toBe('无法使用这个名称')
  })

  test('the share path encodes the handle', () => {
    expect(publicChatPath('rust_learners')).toBe('/public/rust_learners')
  })
})

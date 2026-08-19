import { describe, expect, test } from 'bun:test'
import { credentialInputProps } from './credentialAutocomplete'

describe('credential autocomplete isolation', () => {
  test('keeps account login credentials in the account section', () => {
    expect(credentialInputProps('account-current')).toEqual({
      autocomplete: 'section-user-account current-password',
      minlength: undefined,
      maxlength: 256,
    })
  })

  test('does not expose room access secrets as account passwords', () => {
    const account = credentialInputProps('account-current')
    const room = credentialInputProps('room-access')

    expect(room.autocomplete).toBe('section-chat-room-access new-password')
    expect(room.autocomplete).not.toBe(account.autocomplete)
  })

  test('keeps account password validation on the actual input', () => {
    expect(credentialInputProps('account-new').minlength).toBe(8)
  })
})

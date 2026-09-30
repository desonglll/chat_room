import { describe, expect, test } from 'bun:test'
import { addContactLink } from '../profileApi'

describe('TG-511 QR link', () => {
  test('encodes an absolute add-contact link for the username', () => {
    expect(addContactLink('https://chat.example.org/', 'alice')).toBe('https://chat.example.org/add/alice')
    expect(addContactLink('http://localhost:3000', 'a b')).toBe('http://localhost:3000/add/a%20b')
  })
})

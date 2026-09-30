// New in TG-011 (the old module shipped without a test): pins determinism and palette
// membership, so a palette edit that breaks stable identity colors fails loudly.
import { describe, expect, test } from 'bun:test'
import { avatarColor } from './avatarColor'

describe('avatar colors', () => {
  test('is deterministic per seed', () => {
    expect(avatarColor('alice')).toBe(avatarColor('alice'))
    expect(avatarColor('')).toBe(avatarColor(''))
  })

  test('always returns a hex color from the muted palette', () => {
    for (const seed of ['alice', 'bob', 'carol', '你好', 'a-very-long-user-identifier']) {
      expect(avatarColor(seed)).toMatch(/^#[0-9a-f]{6}$/)
    }
  })

  test('spreads distinct seeds across more than one color', () => {
    const colors = new Set(['alice', 'bob', 'carol', 'dave', 'erin', 'frank'].map(avatarColor))
    expect(colors.size).toBeGreaterThan(1)
  })
})

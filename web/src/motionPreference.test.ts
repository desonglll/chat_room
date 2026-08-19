import { describe, expect, test } from 'bun:test'
import { preferredScrollBehavior } from './motionPreference'

describe('motion preference', () => {
  test('uses immediate scrolling when reduced motion is requested', () => {
    expect(preferredScrollBehavior(true)).toBe('auto')
    expect(preferredScrollBehavior(false)).toBe('smooth')
  })
})

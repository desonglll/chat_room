import { describe, expect, test } from 'bun:test'
import { classifyMessageMotion, classifySystemMotion } from './messageMotion'

describe('message motion', () => {
  test('does not animate initial or prepended history', () => {
    expect(classifyMessageMotion(false, 'user-2', 'user-1')).toBe('none')
    expect(classifySystemMotion(false)).toBe('none')
  })

  test('gives live incoming and outgoing messages distinct motion', () => {
    expect(classifyMessageMotion(true, 'user-2', 'user-1')).toBe('incoming')
    expect(classifyMessageMotion(true, 'user-1', 'user-1')).toBe('outgoing')
    expect(classifySystemMotion(true)).toBe('system')
  })
})

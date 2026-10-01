import { describe, expect, test } from 'bun:test'
import type { KeyInput } from './keymap'
import { globalShortcut, neighbourChat, nextReplyTarget, replyStep } from './keymap'

const key = (value: string, extra: Partial<KeyInput> = {}): KeyInput => ({
  key: value,
  ctrlKey: false,
  metaKey: false,
  altKey: false,
  shiftKey: false,
  editable: false,
  ...extra,
})

describe('TG-606 keyboard map', () => {
  test('search, chat switching, folders and settings', () => {
    expect(globalShortcut(key('k', { ctrlKey: true }))).toEqual({ type: 'search' })
    expect(globalShortcut(key('K', { metaKey: true }))).toEqual({ type: 'search' })
    expect(globalShortcut(key('/'))).toEqual({ type: 'search' })
    expect(globalShortcut(key('/', { editable: true }))).toBeNull()
    expect(globalShortcut(key('ArrowDown', { altKey: true, editable: true }))).toEqual({ type: 'chat', step: 1 })
    expect(globalShortcut(key('ArrowUp', { altKey: true }))).toEqual({ type: 'chat', step: -1 })
    expect(globalShortcut(key('!', { code: 'Digit1', ctrlKey: true, shiftKey: true }))).toEqual({
      type: 'folder',
      index: 1,
    })
    expect(globalShortcut(key(',', { ctrlKey: true }))).toEqual({ type: 'settings' })
    // Browser-owned combinations are left alone.
    expect(globalShortcut(key('1', { code: 'Digit1', ctrlKey: true }))).toBeNull()
    expect(globalShortcut(key('f', { ctrlKey: true }))).toBeNull()
    expect(globalShortcut(key('Tab', { ctrlKey: true }))).toBeNull()
  })

  test('Ctrl/⌘+↑ walks reply targets back from the newest; ↓ past the newest clears it', () => {
    expect(replyStep(key('ArrowUp', { ctrlKey: true, editable: true }))).toBe(-1)
    expect(replyStep(key('ArrowDown', { metaKey: true, editable: true }))).toBe(1)
    expect(replyStep(key('ArrowUp', { editable: true }))).toBeNull()
    const ids = ['m1', 'm2', 'm3']
    expect(nextReplyTarget(ids, null, -1)).toBe('m3')
    expect(nextReplyTarget(ids, 'm3', -1)).toBe('m2')
    expect(nextReplyTarget(ids, 'm1', -1)).toBe('m1')
    expect(nextReplyTarget(ids, 'm2', 1)).toBe('m3')
    expect(nextReplyTarget(ids, 'm3', 1)).toBeNull()
    expect(nextReplyTarget(ids, null, 1)).toBeNull()
    expect(nextReplyTarget([], null, -1)).toBeNull()
  })

  test('chat switching follows the visible list', () => {
    const ids = ['a', 'b', 'c']
    expect(neighbourChat(ids, 'a', 1)).toBe('b')
    expect(neighbourChat(ids, 'c', 1)).toBe('a')
    expect(neighbourChat(ids, 'a', -1)).toBe('c')
    expect(neighbourChat(ids, '', 1)).toBe('a')
    expect(neighbourChat([], 'a', 1)).toBeNull()
  })
})

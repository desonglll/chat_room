// TG-104: markdown shortcut transforms (toggle wrap, code fences, links) and the key map.
import { describe, expect, test } from 'bun:test'
import type { FormatShortcutEvent } from './textFormatting'
import { applyFormat, formatShortcut } from './textFormatting'

const key = (k: string, extra: Partial<FormatShortcutEvent> = {}): FormatShortcutEvent => ({
  key: k,
  ctrlKey: true,
  metaKey: false,
  shiftKey: false,
  altKey: false,
  ...extra,
})

describe('formatShortcut', () => {
  test('Telegram Desktop key map, Ctrl or ⌘', () => {
    expect(formatShortcut(key('b'))).toBe('bold')
    expect(formatShortcut(key('I'))).toBe('italic')
    expect(formatShortcut(key('u', { ctrlKey: false, metaKey: true }))).toBe('underline')
    expect(formatShortcut(key('k'))).toBe('link')
    expect(formatShortcut(key('X', { shiftKey: true }))).toBe('strike')
    expect(formatShortcut(key('m', { shiftKey: true }))).toBe('code')
  })

  test('ignores plain keys, Alt chords and unrelated shortcuts', () => {
    expect(formatShortcut(key('b', { ctrlKey: false }))).toBeNull()
    expect(formatShortcut(key('b', { altKey: true }))).toBeNull()
    expect(formatShortcut(key('c'))).toBeNull()
    expect(formatShortcut(key('b', { shiftKey: true }))).toBeNull()
  })
})

describe('applyFormat', () => {
  test('wraps the selection and keeps it selected', () => {
    expect(applyFormat('say hello now', { start: 4, end: 9 }, 'bold')).toEqual({
      text: 'say **hello** now',
      selection: { start: 6, end: 11 },
    })
    expect(applyFormat('hi', { start: 0, end: 2 }, 'italic').text).toBe('_hi_')
    expect(applyFormat('hi', { start: 0, end: 2 }, 'underline').text).toBe('<u>hi</u>')
    expect(applyFormat('hi', { start: 0, end: 2 }, 'strike').text).toBe('~~hi~~')
    expect(applyFormat('hi', { start: 0, end: 2 }, 'code').text).toBe('`hi`')
  })

  test('toggles: formatting a wrapped selection unwraps it (markers outside or inside)', () => {
    const once = applyFormat('say hello now', { start: 4, end: 9 }, 'bold')
    expect(applyFormat(once.text, once.selection, 'bold')).toEqual({
      text: 'say hello now',
      selection: { start: 4, end: 9 },
    })
    expect(applyFormat('**hi**', { start: 0, end: 6 }, 'bold')).toEqual({ text: 'hi', selection: { start: 0, end: 2 } })
  })

  test('trims whitespace out of the selection so markdown still parses', () => {
    expect(applyFormat('a  word  b', { start: 1, end: 8 }, 'bold').text).toBe('a  **word**  b')
  })

  test('an empty selection inserts a marker pair with the caret inside', () => {
    expect(applyFormat('ab', { start: 1, end: 1 }, 'bold')).toEqual({ text: 'a****b', selection: { start: 3, end: 3 } })
  })

  test('multi-line code becomes a fenced block on its own lines', () => {
    const result = applyFormat('see\nx = 1\ny = 2', { start: 4, end: 15 }, 'code')
    expect(result.text).toBe('see\n```\nx = 1\ny = 2\n```')
    expect(result.text.slice(result.selection.start, result.selection.end)).toBe('x = 1\ny = 2')
  })

  test('link: label from selection; url slot selected when no url given', () => {
    const blank = applyFormat('go docs', { start: 3, end: 7 }, 'link')
    expect(blank.text).toBe('go [docs](https://)')
    expect(blank.text.slice(blank.selection.start, blank.selection.end)).toBe('https://')
    const withUrl = applyFormat('go docs', { start: 3, end: 7 }, 'link', ' https://x.dev ')
    expect(withUrl).toEqual({ text: 'go [docs](https://x.dev)', selection: { start: 24, end: 24 } })
  })

  test('reversed and out-of-range selections are clamped', () => {
    expect(applyFormat('abc', { start: 9, end: 1 }, 'italic').text).toBe('a_bc_')
  })
})

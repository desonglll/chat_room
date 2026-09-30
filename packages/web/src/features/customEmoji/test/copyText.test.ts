import { describe, expect, test } from 'bun:test'
import { customEmojiPlainText, type CopyNode } from '../copyText'

const text = (value: string): CopyNode => ({ nodeType: 3, nodeName: '#text', nodeValue: value, childNodes: [] })
const element = (name: string, attributes: Record<string, string>, ...children: CopyNode[]): CopyNode => ({
  nodeType: 1,
  nodeName: name,
  nodeValue: null,
  childNodes: children,
  getAttribute: (attribute) => attributes[attribute] ?? null,
})
const fragment = (...children: CopyNode[]): CopyNode => ({
  nodeType: 11,
  nodeName: '#document-fragment',
  nodeValue: null,
  childNodes: children,
})
const customEmoji = (fallback: string) =>
  element('SPAN', { 'data-custom-emoji': fallback, role: 'img' }, element('IMG', { alt: fallback, src: '/x' }))

describe('customEmojiPlainText', () => {
  test('a selection with custom emoji copies their fallback emoji, not an image or a placeholder', () => {
    const selection = fragment(element('SPAN', {}, text('hi ')), customEmoji('😺'), element('SPAN', {}, text('!')))
    expect(customEmojiPlainText(selection)).toBe('hi 😺!')
  })

  test('an unresolved custom emoji (fallback text child) is not doubled', () => {
    const unresolved = element('SPAN', { 'data-custom-emoji': '😸' }, text('😸'))
    expect(customEmojiPlainText(fragment(unresolved, unresolved))).toBe('😸😸')
  })

  test('a selection without custom emoji is left to the browser', () => {
    expect(customEmojiPlainText(fragment(element('SPAN', {}, text('plain'))))).toBeNull()
  })

  test('selections across bubbles keep a line break between text blocks', () => {
    const bubble = (...children: CopyNode[]) => element('DIV', { class: 'tg-bubble__text' }, ...children)
    expect(customEmojiPlainText(fragment(bubble(text('one '), customEmoji('⭐')), bubble(text('two'))))).toBe(
      'one ⭐\ntwo',
    )
  })
})

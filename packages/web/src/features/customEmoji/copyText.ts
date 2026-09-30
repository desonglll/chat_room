/**
 * Copying text that contains custom emoji. An inline custom emoji is an `<img>` (or an
 * animation) whose text is its fallback emoji; browsers disagree on whether a copied image
 * contributes its `alt` to `text/plain` (Chrome drops it). So when a selection holds any
 * custom emoji, the copy handler writes the plain text itself, with every custom emoji
 * replaced by its fallback — the same text the sender typed.
 */
import type { ClipboardEvent } from 'react'

/** The attribute that marks an inline custom emoji and carries its fallback text. */
export const CUSTOM_EMOJI_ATTRIBUTE = 'data-custom-emoji'

/** The slice of a DOM node the text walk reads — a real `Node` satisfies it. */
export interface CopyNode {
  nodeType: number
  nodeName: string
  nodeValue: string | null
  childNodes: ArrayLike<CopyNode>
  getAttribute?(name: string): string | null
}

const TEXT_NODE = 3
const ELEMENT_NODE = 1
const BLOCKS = new Set(['DIV', 'P', 'LI'])

function walk(node: CopyNode, out: string[]): boolean {
  let found = false
  if (node.nodeType === TEXT_NODE) {
    out.push(node.nodeValue ?? '')
    return false
  }
  if (node.nodeType === ELEMENT_NODE) {
    const fallback = node.getAttribute?.(CUSTOM_EMOJI_ATTRIBUTE)
    if (fallback != null) {
      out.push(fallback)
      return true
    }
    if (node.nodeName === 'BR') {
      out.push('\n')
      return false
    }
    if (node.nodeName === 'IMG') {
      out.push(node.getAttribute?.('alt') ?? '')
      return false
    }
    if (BLOCKS.has(node.nodeName) && out.length > 0 && !out[out.length - 1]!.endsWith('\n')) out.push('\n')
  }
  for (let index = 0; index < node.childNodes.length; index++) {
    if (walk(node.childNodes[index]!, out)) found = true
  }
  return found
}

/** Plain text of a copied fragment, custom emoji as their fallback; `null` when it holds none. */
export function customEmojiPlainText(root: CopyNode): string | null {
  const out: string[] = []
  return walk(root, out) ? out.join('') : null
}

/** `onCopy` for any element that renders inline custom emoji. */
export function handleCustomEmojiCopy(event: ClipboardEvent<HTMLElement>): void {
  const selection = event.currentTarget.ownerDocument.getSelection()
  if (selection === null || selection.rangeCount === 0 || selection.isCollapsed) return
  const text = customEmojiPlainText(selection.getRangeAt(0).cloneContents())
  if (text === null) return // nothing custom selected: the browser's own copy is right
  event.preventDefault()
  event.clipboardData.setData('text/plain', text)
}

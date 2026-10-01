import { t } from '../i18n/t'
/**
 * TG-104: markdown formatting shortcuts and the selection toolbar's transforms.
 *
 * Messages are rendered by the frozen markdown policy (`markdown.ts`: GFM, then DOMPurify
 * with the html profile), so the composer writes plain markdown the renderer already
 * understands. Markdown has no underline; `<u>` survives the html sanitise profile, so
 * underline uses it. Each transform TOGGLES: formatting an already-wrapped selection
 * unwraps it, which is what makes Ctrl+B pressed twice a no-op like in Telegram.
 *
 * Keyboard map follows Telegram Desktop: Ctrl/⌘+B bold, +I italic, +U underline,
 * Ctrl+Shift+X strikethrough, Ctrl+Shift+M monospace, Ctrl+K link.
 */

export type FormatKind = 'bold' | 'italic' | 'underline' | 'strike' | 'code' | 'link'

export interface TextSelection {
  start: number
  end: number
}

export interface FormattedText {
  text: string
  selection: TextSelection
}

export interface FormatShortcutEvent {
  key: string
  ctrlKey: boolean
  metaKey: boolean
  shiftKey: boolean
  altKey: boolean
}

const MARKERS: Record<Exclude<FormatKind, 'link' | 'code'>, readonly [string, string]> = {
  bold: ['**', '**'],
  italic: ['_', '_'],
  underline: ['<u>', '</u>'],
  strike: ['~~', '~~'],
}

export function formatShortcut(event: FormatShortcutEvent): FormatKind | null {
  if (!(event.ctrlKey || event.metaKey) || event.altKey) return null
  const key = event.key.toLowerCase()
  if (event.shiftKey) {
    if (key === 'x') return 'strike'
    if (key === 'm') return 'code'
    return null
  }
  if (key === 'b') return 'bold'
  if (key === 'i') return 'italic'
  if (key === 'u') return 'underline'
  if (key === 'k') return 'link'
  return null
}

/** Shrink a selection past leading/trailing whitespace: `** bold**` is not bold in markdown. */
function trimSelection(text: string, selection: TextSelection): TextSelection {
  let { start, end } = selection
  while (start < end && /\s/.test(text[start]!)) start += 1
  while (end > start && /\s/.test(text[end - 1]!)) end -= 1
  return { start, end }
}

function toggleWrap(text: string, raw: TextSelection, open: string, close: string): FormattedText {
  const selection = trimSelection(text, raw)
  const { start, end } = selection
  const inner = text.slice(start, end)
  // Markers just outside the selection: `**[word]**` → unwrap.
  if (text.slice(start - open.length, start) === open && text.slice(end, end + close.length) === close) {
    const next = text.slice(0, start - open.length) + inner + text.slice(end + close.length)
    return { text: next, selection: { start: start - open.length, end: end - open.length } }
  }
  // Markers selected along with the word: `[**word**]` → unwrap.
  if (inner.length >= open.length + close.length && inner.startsWith(open) && inner.endsWith(close)) {
    const unwrapped = inner.slice(open.length, inner.length - close.length)
    return {
      text: text.slice(0, start) + unwrapped + text.slice(end),
      selection: { start, end: start + unwrapped.length },
    }
  }
  const next = text.slice(0, start) + open + inner + close + text.slice(end)
  return { text: next, selection: { start: start + open.length, end: end + open.length } }
}

function formatCode(text: string, selection: TextSelection): FormattedText {
  const inner = text.slice(selection.start, selection.end)
  if (!inner.includes('\n')) return toggleWrap(text, selection, '`', '`')
  // A multi-line selection becomes a fenced block on its own lines.
  const before = text.slice(0, selection.start)
  const after = text.slice(selection.end)
  const open = `${before && !before.endsWith('\n') ? '\n' : ''}\`\`\`\n`
  const close = `\n\`\`\`${after && !after.startsWith('\n') ? '\n' : ''}`
  const start = before.length + open.length
  return { text: before + open + inner + close + after, selection: { start, end: start + inner.length } }
}

/**
 * `[label](url)`. With a url the caret lands after the link; without one the url slot
 * is selected so the user can type it straight away.
 */
function formatLink(text: string, raw: TextSelection, url: string): FormattedText {
  const selection = trimSelection(text, raw)
  const label = text.slice(selection.start, selection.end) || url || t('c.domain.715022')
  const target = url || 'https://'
  const link = `[${label}](${target})`
  const next = text.slice(0, selection.start) + link + text.slice(selection.end)
  if (url) {
    const caret = selection.start + link.length
    return { text: next, selection: { start: caret, end: caret } }
  }
  const urlStart = selection.start + label.length + 3
  return { text: next, selection: { start: urlStart, end: urlStart + target.length } }
}

export function applyFormat(text: string, selection: TextSelection, kind: FormatKind, url = ''): FormattedText {
  const clamped = {
    start: Math.max(0, Math.min(selection.start, selection.end, text.length)),
    end: Math.min(text.length, Math.max(selection.start, selection.end)),
  }
  if (kind === 'link') return formatLink(text, clamped, url.trim())
  if (kind === 'code') return formatCode(text, clamped)
  const [open, close] = MARKERS[kind]
  return toggleWrap(text, clamped, open, close)
}

/**
 * Split plain message text into text and link segments. Only absolute http(s) URLs become
 * links — the scheme check is what keeps `javascript:` out, so it is not optional. Trailing
 * sentence punctuation stays text ("see https://x.io." links `https://x.io`).
 */
export type TextSegment = { type: 'text'; text: string } | { type: 'link'; text: string; href: string }

const URL_PATTERN = /\bhttps?:\/\/[^\s<>"']+/gi
const TRAILING_PUNCTUATION = /[.,;:!?)\]}'"，。；：！？）】」』]+$/

export function linkify(text: string): TextSegment[] {
  const segments: TextSegment[] = []
  let cursor = 0
  for (const match of text.matchAll(URL_PATTERN)) {
    const start = match.index
    const raw = match[0]
    const href = raw.replace(TRAILING_PUNCTUATION, '')
    if (href.length === 0) continue
    if (start > cursor) segments.push({ type: 'text', text: text.slice(cursor, start) })
    segments.push({ type: 'link', text: href, href })
    cursor = start + href.length
  }
  if (cursor < text.length) segments.push({ type: 'text', text: text.slice(cursor) })
  return segments
}

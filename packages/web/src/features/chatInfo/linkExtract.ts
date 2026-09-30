/**
 * Links out of message text, for the "链接" tab. Pure.
 *
 * The server has no link index (backend gap recorded in docs/devlog/TG-106.md), so the
 * tab derives links from the chat's own message search: `q=http` narrows the rows
 * server-side (authorised, keyset-paginated), and this module pulls the URLs out with
 * the bubble's own `linkify` — the tab lists exactly what the bubble renders as a link.
 */
import { linkify } from '../message/content/linkify'

export interface ExtractedLink {
  url: string
  host: string
}

export function extractLinks(text: string): ExtractedLink[] {
  const found: ExtractedLink[] = []
  const seen = new Set<string>()
  for (const segment of linkify(text)) {
    if (segment.type !== 'link' || seen.has(segment.href)) continue
    let host: string
    try {
      host = new URL(segment.href).hostname.replace(/^www\./, '')
    } catch {
      continue
    }
    if (!host) continue
    seen.add(segment.href)
    found.push({ url: segment.href, host })
  }
  return found
}

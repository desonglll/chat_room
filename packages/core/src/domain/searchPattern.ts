/**
 * The client-side mirror of the server's message-search matching
 * (`src/messages/search_pattern.rs::like_pattern`): SQL LIKE with `\`-escaped wildcards,
 * wrapped in `%…%`, matched case-insensitively for ASCII (SQLite LIKE semantics; the
 * PostgreSQL adapter uses ILIKE). The client needs the same rules to pre-filter locally
 * and to highlight excerpts consistently with what the server returned.
 */

/** Exactly `like_pattern(text)` from `src/messages/search_pattern.rs`. */
export function likePattern(text: string): string {
  const escaped = text.replaceAll('\\', '\\\\').replaceAll('%', '\\%').replaceAll('_', '\\_')
  return `%${escaped}%`
}

/** The substring predicate `like_pattern` compiles to: case-insensitive contains. */
export function textMatchesSearch(text: string, query: string): boolean {
  if (!query) return false
  return text.toLowerCase().includes(query.toLowerCase())
}

export interface SearchMatchRange {
  start: number
  end: number
}

/** Every non-overlapping match of `query` in `text`, for excerpt highlighting. */
export function searchMatchRanges(text: string, query: string): SearchMatchRange[] {
  if (!query) return []
  const haystack = text.toLowerCase()
  const needle = query.toLowerCase()
  const ranges: SearchMatchRange[] = []
  let from = 0
  while (from <= haystack.length - needle.length) {
    const start = haystack.indexOf(needle, from)
    if (start < 0) break
    ranges.push({ start, end: start + needle.length })
    from = start + needle.length
  }
  return ranges
}

/**
 * TG-104: @mention autocomplete — find the token under the caret, rank chat members
 * against it, and splice the chosen username back in. Pure string logic; the composer
 * owns the caret and the popup.
 *
 * Telegram rules adopted:
 * - a mention starts at `@` that opens the text or follows whitespace (so `a@b.c` is an
 *   e-mail, not a mention) and runs to the caret without whitespace;
 * - an empty query (`@` alone) lists everyone;
 * - prefix matches rank above substring matches, then shorter names, then alphabetical;
 * - the current user is never suggested.
 */

export interface MentionQuery {
  /** Index of the `@`. */
  start: number
  /** The caret — the end of the token being typed. */
  end: number
  /** Text between `@` and the caret, as typed. */
  query: string
}

export interface MentionCandidate {
  user_id: string
  username: string
}

/** Telegram usernames are ≤ 32 chars; a longer run is prose, not a mention. */
export const MAX_MENTION_QUERY = 32
export const DEFAULT_MENTION_LIMIT = 8

const WHITESPACE = /\s/

export function findMentionQuery(text: string, caret: number): MentionQuery | null {
  const end = Math.max(0, Math.min(caret, text.length))
  for (let index = end - 1; index >= 0 && end - index <= MAX_MENTION_QUERY + 1; index -= 1) {
    const char = text[index]!
    if (WHITESPACE.test(char)) return null
    if (char === '@') {
      const before = index === 0 ? '' : text[index - 1]!
      if (before !== '' && !WHITESPACE.test(before)) return null
      return { start: index, end, query: text.slice(index + 1, end) }
    }
  }
  return null
}

export function matchMentionCandidates<T extends MentionCandidate>(
  members: readonly T[],
  query: string,
  options: { excludeUserId?: string; limit?: number } = {},
): T[] {
  const needle = query.toLocaleLowerCase()
  const limit = options.limit ?? DEFAULT_MENTION_LIMIT
  const seen = new Set<string>()
  const scored: Array<{ member: T; rank: number }> = []
  for (const member of members) {
    if (!member.username || member.user_id === options.excludeUserId || seen.has(member.user_id)) continue
    const name = member.username.toLocaleLowerCase()
    const at = name.indexOf(needle)
    if (at < 0) continue
    seen.add(member.user_id)
    scored.push({ member, rank: at === 0 ? 0 : 1 })
  }
  scored.sort(
    (a, b) =>
      a.rank - b.rank ||
      a.member.username.length - b.member.username.length ||
      a.member.username.localeCompare(b.member.username),
  )
  return scored.slice(0, limit).map((entry) => entry.member)
}

/** Replace `@query` with `@username ` and put the caret after the space. */
export function applyMention(text: string, mention: MentionQuery, username: string): { text: string; caret: number } {
  const inserted = `@${username} `
  const after = text.slice(mention.end)
  // Do not double the separator when the user had already typed one.
  const tail = after.startsWith(' ') ? after.slice(1) : after
  return {
    text: text.slice(0, mention.start) + inserted + tail,
    caret: mention.start + inserted.length,
  }
}

/**
 * Initials for `Avatar`'s image-less state.
 *
 * Generic on purpose: the input is a `label`, not a user or a chat. Grapheme-aware enough to not
 * split an emoji or a surrogate pair in half, which naive `charAt(0)` does.
 */
export function initialsFrom(label: string, maxLetters = 2): string {
  const words = label.trim().split(/\s+/u).filter(Boolean)
  if (words.length === 0) return ''
  const picked = words.length === 1 ? [words[0] as string] : [words[0] as string, words[words.length - 1] as string]
  return picked
    .slice(0, Math.max(1, maxLetters))
    .map((word) => Array.from(word)[0] ?? '')
    .join('')
    .toUpperCase()
}

/**
 * Deterministic palette slot from a label, for callers that have no explicit `colorIndex`.
 * A stable hash, not randomness, so the same label always gets the same swatch across reloads
 * and across devices.
 */
export function paletteIndex(label: string, slots: number): number {
  if (slots <= 0) return 0
  let hash = 0
  for (const codePoint of label) {
    hash = (hash * 31 + (codePoint.codePointAt(0) ?? 0)) % 0x7fffffff
  }
  return hash % slots
}

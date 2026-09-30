/** Pure helpers for the management page: reorder and the public share link. */

/** `ids` with the element at `from` moved to index `to` (both clamped). */
export function moveId(ids: readonly string[], from: number, to: number): string[] {
  const next = [...ids]
  if (from < 0 || from >= next.length) return next
  const target = Math.max(0, Math.min(next.length - 1, to))
  const [moved] = next.splice(from, 1)
  next.splice(target, 0, moved!)
  return next
}

/** Telegram's `t.me/addstickers/<name>` on this deployment's origin. */
export function stickerSetLink(origin: string, shortName: string): string {
  return `${origin.replace(/\/+$/, '')}/addstickers/${encodeURIComponent(shortName)}`
}

/** `/addstickers/<name>` or a full link to it → the short name, else `null`. */
export function shortNameFromLink(input: string): string | null {
  const match = /(?:^|\/)addstickers\/([A-Za-z][A-Za-z0-9_]{0,63})\/?(?:[?#].*)?$/.exec(input.trim())
  if (match) return match[1]!.toLowerCase()
  const bare = input.trim()
  return /^[A-Za-z][A-Za-z0-9_]{0,63}$/.test(bare) ? bare.toLowerCase() : null
}

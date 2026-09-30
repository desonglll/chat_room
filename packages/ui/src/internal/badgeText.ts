/**
 * Counter formatting for `Badge`. Telegram caps the visible number and appends a plus rather
 * than letting a four-digit unread count widen a chat row.
 */
export interface CountFormat {
  /** What the badge paints. */
  text: string
  /** What assistive technology should hear: the exact number, never the truncated form. */
  exact: string
}

export function formatCount(count: number, max: number): CountFormat {
  const safe = Number.isFinite(count) ? Math.max(0, Math.trunc(count)) : 0
  const limit = Number.isFinite(max) ? Math.max(0, Math.trunc(max)) : 0
  const text = limit > 0 && safe > limit ? `${limit}+` : String(safe)
  return { text, exact: String(safe) }
}

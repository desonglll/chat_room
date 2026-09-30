/**
 * "Is the whole draft exactly one emoji?" — the trigger for Telegram's sticker suggestions.
 *
 * One emoji is one of: a regional-indicator pair (flag); a keycap (`1️⃣`); a tag sequence
 * (subdivision flags); or a pictographic base with an optional variation selector and skin
 * tone, joined by ZWJ to further such bases (families, professions). Anything else —
 * text, two emoji, an emoji plus a space inside — is `null`. Surrounding whitespace is
 * ignored, as Telegram does.
 */
const PICTO = '\\p{Extended_Pictographic}(?:\\uFE0F|\\uFE0E)?(?:\\p{Emoji_Modifier})?'
const ONE_EMOJI = new RegExp(
  [
    '^(?:',
    '\\p{Regional_Indicator}{2}',
    '|[0-9#*]\\uFE0F?\\u20E3',
    `|${PICTO}(?:\\u200D${PICTO})*`,
    '(?:[\\u{E0020}-\\u{E007E}]+\\u{E007F})?',
    ')$',
  ].join(''),
  'u',
)

/** Longest draft worth testing: the longest RGI ZWJ sequences are well under this. */
const MAX_EMOJI_LENGTH = 32

export function singleEmoji(draft: string): string | null {
  if (draft.length === 0 || draft.length > MAX_EMOJI_LENGTH + 8) return null
  const trimmed = draft.trim()
  if (trimmed.length === 0 || trimmed.length > MAX_EMOJI_LENGTH) return null
  return ONE_EMOJI.test(trimmed) ? trimmed : null
}

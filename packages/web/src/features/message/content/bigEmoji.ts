/**
 * TG-411: Telegram draws a text message that is nothing but one to three emoji without a
 * bubble, enlarged. This decides whether a text qualifies and how many there are (the size
 * steps down as the count goes up). Whitespace between them is allowed; anything else is not.
 */
export const BIG_EMOJI_MAX = 3

const PICTOGRAPHIC = /\p{Extended_Pictographic}|\p{Regional_Indicator}/u
const segmenter =
  typeof Intl !== 'undefined' && 'Segmenter' in Intl ? new Intl.Segmenter(undefined, { granularity: 'grapheme' }) : null

/** 1–3 when `text` is only emoji (and whitespace), otherwise 0. */
export function bigEmojiCount(text: string): number {
  if (segmenter === null || text.length === 0 || text.length > 64) return 0
  let count = 0
  for (const { segment } of segmenter.segment(text)) {
    if (/^\s+$/u.test(segment)) continue
    // A keycap digit (1️⃣) contains a digit, so test for the pictographic part, not "no letters".
    if (!PICTOGRAPHIC.test(segment) && !segment.includes('⃣')) return 0
    count += 1
    if (count > BIG_EMOJI_MAX) return 0
  }
  return count
}

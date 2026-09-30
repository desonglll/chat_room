/**
 * Emoji → sticker suggestions above the input. Pure and clock-driven, so the ≤ 200 ms
 * acceptance bound is a unit test with a fake clock, not a hope.
 *
 * On every draft change `update(draft)` either hides the strip at once (the draft is no
 * longer one emoji) or schedules a lookup `delayMs` later. The short delay absorbs the
 * intermediate states of a multi-code-point emoji arriving from an IME, which would
 * otherwise flash a strip for "👩" on the way to "👩‍💻". The lookup is a Map read
 * (`suggestStickers`), so appearance time ≈ `delayMs`.
 *
 * `dismiss()` (Esc, or the strip's close) hides the strip until the draft changes.
 */
import type { CoreClock, CoreTimerHandle, Sticker } from '@tg/core'
import { singleEmoji } from './singleEmoji'

export const SUGGESTION_DELAY_MS = 60

export interface Suggestions {
  emoji: string
  stickers: readonly Sticker[]
}

export const NO_SUGGESTIONS: Suggestions = Object.freeze({
  emoji: '',
  stickers: Object.freeze([]) as readonly Sticker[],
})

export interface SuggestionControllerDeps {
  clock: Pick<CoreClock, 'setTimeout' | 'clearTimeout'>
  lookup(emoji: string): readonly Sticker[]
  onChange(suggestions: Suggestions): void
  delayMs?: number
}

export interface SuggestionController {
  update(draft: string): void
  /** The library changed (sets loaded, favorite added): recompute what is shown. */
  refresh(): void
  dismiss(): void
  dispose(): void
}

export function createSuggestionController(deps: SuggestionControllerDeps): SuggestionController {
  const delay = deps.delayMs ?? SUGGESTION_DELAY_MS
  let timer: CoreTimerHandle | null = null
  let draft = ''
  let dismissedFor: string | null = null
  let shown: Suggestions = NO_SUGGESTIONS

  const show = (next: Suggestions) => {
    if (next.emoji === shown.emoji && next.stickers.length === shown.stickers.length) {
      if (next.stickers.every((sticker, index) => sticker.id === shown.stickers[index]?.id)) return
    }
    shown = next
    deps.onChange(next)
  }
  const cancel = () => {
    if (timer !== null) deps.clock.clearTimeout(timer)
    timer = null
  }
  const compute = () => {
    timer = null
    const emoji = singleEmoji(draft)
    if (emoji === null || dismissedFor === draft) return show(NO_SUGGESTIONS)
    const stickers = deps.lookup(emoji)
    show(stickers.length === 0 ? NO_SUGGESTIONS : { emoji, stickers })
  }

  return {
    update(next) {
      if (next === draft) return
      draft = next
      dismissedFor = null
      cancel()
      if (singleEmoji(next) === null) {
        show(NO_SUGGESTIONS)
        return
      }
      timer = deps.clock.setTimeout(compute, delay)
    },
    refresh() {
      if (timer === null) compute()
    },
    dismiss() {
      cancel()
      dismissedFor = draft
      show(NO_SUGGESTIONS)
    },
    dispose: cancel,
  }
}

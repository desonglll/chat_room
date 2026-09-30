import { describe, expect, test } from 'bun:test'
import { singleEmoji } from '../suggest/singleEmoji'
import { createSuggestionController, SUGGESTION_DELAY_MS, type Suggestions } from '../suggest/suggestionController'
import { FakeClock, makeSticker } from './fixtures'

describe('singleEmoji', () => {
  test('accepts one emoji of every shape, with surrounding whitespace', () => {
    for (const emoji of ['😀', '❤️', '❤', '👍🏽', '👩‍💻', '👨‍👩‍👧‍👦', '🇨🇳', '1️⃣', '🏴󠁧󠁢󠁳󠁣󠁴󠁿', '🏳️‍🌈']) {
      expect(singleEmoji(emoji)).toBe(emoji)
    }
    expect(singleEmoji('  😀 ')).toBe('😀')
  })

  test('rejects text, two emoji and empty drafts', () => {
    for (const draft of ['', ' ', 'hi', 'hi 😀', '😀😀', '😀 😀', '1', '#', '©abc']) {
      expect(singleEmoji(draft)).toBeNull()
    }
  })
})

describe('suggestion controller', () => {
  const setup = (lookup = (emoji: string) => (emoji === '😀' ? [makeSticker('a'), makeSticker('b')] : [])) => {
    const clock = new FakeClock()
    const shown: Suggestions[] = []
    const controller = createSuggestionController({ clock, lookup, onChange: (next) => shown.push(next) })
    return { clock, shown, controller }
  }

  test('suggestions appear within 200 ms of the keystroke that made the draft one emoji', () => {
    const { clock, shown, controller } = setup()
    const keystroke = clock.now()
    controller.update('😀')
    expect(shown).toEqual([])
    clock.advance(SUGGESTION_DELAY_MS)
    expect(shown.at(-1)?.stickers.map((sticker) => sticker.id)).toEqual(['a', 'b'])
    expect(clock.now() - keystroke).toBeLessThanOrEqual(200)
  })

  test('typing past the emoji hides the strip at once, not after the delay', () => {
    const { clock, shown, controller } = setup()
    controller.update('😀')
    clock.advance(SUGGESTION_DELAY_MS)
    controller.update('😀 hi')
    expect(shown.at(-1)?.stickers).toEqual([])
    expect(clock.pending()).toBe(0)
  })

  test('intermediate IME states inside the delay never flash a strip', () => {
    const calls: string[] = []
    const { clock, controller } = setup((emoji) => {
      calls.push(emoji)
      return [makeSticker(emoji)]
    })
    controller.update('👩')
    clock.advance(20)
    controller.update('👩‍💻')
    clock.advance(SUGGESTION_DELAY_MS)
    expect(calls).toEqual(['👩‍💻'])
  })

  test('dismiss hides until the draft changes; refresh picks up a library that loaded late', () => {
    let stickers = [] as ReturnType<typeof makeSticker>[]
    const { clock, shown, controller } = setup(() => stickers)
    controller.update('😀')
    clock.advance(SUGGESTION_DELAY_MS)
    expect(shown).toEqual([])
    stickers = [makeSticker('late')]
    controller.refresh()
    expect(shown.at(-1)?.stickers.map((sticker) => sticker.id)).toEqual(['late'])
    controller.dismiss()
    expect(shown.at(-1)?.stickers).toEqual([])
    controller.refresh()
    expect(shown.at(-1)?.stickers).toEqual([])
    controller.update('😀 ')
    clock.advance(SUGGESTION_DELAY_MS)
    expect(shown.at(-1)?.stickers.map((sticker) => sticker.id)).toEqual(['late'])
  })
})

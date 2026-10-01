import { describe, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { ReactionRow } from '../BubbleParts'
import { bigEmojiCount } from '../content/bigEmoji'
import { messageContent } from '../content/messageContent'
import { makeMessage, photo } from '../fixtures/bubbleFixtures'

describe('TG-411 big emoji', () => {
  test('one to three emoji alone qualify; text, four emoji or nothing do not', () => {
    expect(bigEmojiCount('👍')).toBe(1)
    expect(bigEmojiCount('😂 🎉')).toBe(2)
    expect(bigEmojiCount('👨‍👩‍👧🇨🇳1️⃣')).toBe(3)
    expect(bigEmojiCount('👍👍👍👍')).toBe(0)
    expect(bigEmojiCount('ok 👍')).toBe(0)
    expect(bigEmojiCount('1')).toBe(0)
    expect(bigEmojiCount('')).toBe(0)
  })

  test('an emoji-only text message draws bare and large; with media or formatting it stays a bubble', () => {
    const kind = messageContent.resolve(makeMessage({ content: '🎉' }))
    expect(kind?.kind).toBe('bigEmoji')
    expect(kind?.frame(makeMessage({ content: '🎉' }))).toBe('bare')
    expect(messageContent.resolve(makeMessage({ content: 'hi 🎉' }))?.kind).toBe('text')
    expect(messageContent.resolve(makeMessage({ content: '🎉', attachment: photo('p1', 'p') }))?.kind).not.toBe(
      'bigEmoji',
    )
    const Component = kind!.component
    const html = renderToStaticMarkup(
      <Component message={makeMessage({ content: '🎉' })} ctx={{} as never} actions={{} as never} metaSpacer={null} />,
    )
    expect(html).toContain('data-count="1"')
    expect(html).toContain('aria-label="🎉"')
  })
})

describe('TG-411 reaction burst', () => {
  test('chips that were already chosen when they mounted do not burst', () => {
    const html = renderToStaticMarkup(
      <ReactionRow
        reactions={[{ emoji: '❤️', user_ids: ['me'] }] as never}
        viewerId="me"
        onReact={() => {}}
        metaSpacer={null}
      />,
    )
    expect(html).toContain('aria-pressed="true"')
    expect(html).not.toContain('data-burst')
  })
})

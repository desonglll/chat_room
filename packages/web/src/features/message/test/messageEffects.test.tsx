import { describe, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { ReactionRow } from '../BubbleParts'
import { isFreshOwnReaction, noteOwnReaction } from '../reactionIntent'
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
        messageId="history"
        reactions={[{ emoji: '❤️', user_ids: ['me'] }] as never}
        viewerId="me"
        onReact={() => {}}
        metaSpacer={null}
      />,
    )
    expect(html).toContain('aria-pressed="true"')
    expect(html).not.toContain('data-burst')
  })

  // TG-1201: the first reaction to a message mounts a new chip that is chosen from the start;
  // without the viewer's tap note it could never burst, so Telegram's most common case was flat.
  test('a chip mounted by the viewer’s own first reaction bursts; others on that message do not', () => {
    noteOwnReaction('m-first', '👍')
    const html = renderToStaticMarkup(
      <ReactionRow
        messageId="m-first"
        reactions={
          [
            { emoji: '👍', user_ids: ['me'] },
            { emoji: '🔥', user_ids: ['me', 'other'] },
          ] as never
        }
        viewerId="me"
        onReact={() => {}}
        metaSpacer={null}
      />,
    )
    expect(html.match(/data-burst/g)?.length).toBe(1)
    expect(html).toMatch(/data-burst=""[^>]*>[\s\S]*?👍/)
  })

  test('the tap note expires, so a later remount of the same chip stays still', () => {
    noteOwnReaction('m-old', '👍', 1_000)
    expect(isFreshOwnReaction('m-old', '👍', 3_500)).toBe(true)
    expect(isFreshOwnReaction('m-old', '👍', 4_500)).toBe(false)
    expect(isFreshOwnReaction('m-old', '🔥', 1_500)).toBe(false)
  })
})

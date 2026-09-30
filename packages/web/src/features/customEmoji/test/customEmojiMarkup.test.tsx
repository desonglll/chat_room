/**
 * Server-rendered markup of the TG-304 components (packages/web has no DOM under bun test).
 */
import { afterEach, beforeEach, describe, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import type { CustomEmojiSet, EmojiStatus as EmojiStatusValue } from '@tg/core'
import { MessageBubble, messageContent } from '../../message'
import { makeCtx, makeMessage } from '../../message/fixtures/bubbleFixtures'
import { CustomEmojiGrid } from '../CustomEmojiGrid'
import { EmojiStatus } from '../EmojiStatus'
import { EntityText } from '../EntityText'
import { registerAnimatedEmojiRenderer } from '../animatedRenderers'
import { unregisterEntityText } from '../register'
import { configureCustomEmoji } from '../services'
import { NOW, emojiFixture, installFakeServices } from './fakeServices'

const entity = (offset: number, length: number, id: string) => ({
  type: 'custom_emoji',
  offset,
  length,
  custom_emoji_id: id,
})

let services: ReturnType<typeof installFakeServices>
beforeEach(() => {
  services = installFakeServices()
})
afterEach(() => configureCustomEmoji(null))

describe('EntityText', () => {
  test('a resolved emoji is an image whose alt and copy marker are the fallback emoji', () => {
    services.emoji.seed([['e1', emojiFixture('e1')]])
    const html = renderToStaticMarkup(<EntityText text="hi 😺!" entities={[entity(3, 2, 'e1')]} />)
    expect(html).toContain('data-custom-emoji="😺"')
    expect(html).toContain('alt="😺"')
    expect(html).toContain('src="/api/stickers/e1/file?key=k"')
    expect(html).toContain('<span>hi </span>')
    expect(html).toContain('<span>!</span>')
  })

  test('unknown, removed and not-yet-loaded emoji fall back to the Unicode emoji', () => {
    services.emoji.seed([['gone', null]])
    const html = renderToStaticMarkup(
      <EntityText text="😺😸" entities={[entity(0, 2, 'gone'), entity(2, 2, 'later')]} />,
    )
    expect(html).not.toContain('<img')
    expect(html).toContain('aria-label="😺">😺</span>')
    expect(html).toContain('aria-label="😸">😸</span>')
  })

  test('an animated emoji uses the registered renderer, else its fallback', () => {
    services.emoji.seed([['a1', emojiFixture('a1', '🔥', 'tgs')]])
    const text = '🔥'
    expect(renderToStaticMarkup(<EntityText text={text} entities={[entity(0, 2, 'a1')]} />)).toContain(
      'aria-label="🔥">🔥</span>',
    )
    const undo = registerAnimatedEmojiRenderer('tgs', ({ src }) => <canvas data-src={src} />)
    try {
      expect(renderToStaticMarkup(<EntityText text={text} entities={[entity(0, 2, 'a1')]} />)).toContain(
        'data-src="/api/stickers/a1/file?key=k"',
      )
    } finally {
      undo()
    }
  })

  test('a stale entity over ordinary text renders the plain text', () => {
    const html = renderToStaticMarkup(<EntityText text="hello" entities={[entity(0, 2, 'e1')]} />)
    expect(html).not.toContain('data-custom-emoji')
    expect(html).toContain('hello')
  })
})

describe('message content registration', () => {
  test('text messages with custom emoji resolve to entity_text; others keep their kinds', () => {
    const withEmoji = makeMessage({ content: '😺', entities: [entity(0, 2, 'e1')] })
    expect(messageContent.resolve(withEmoji)?.kind).toBe('entity_text')
    expect(messageContent.resolve(makeMessage({ content: 'plain' }))?.kind).toBe('text')
    expect(messageContent.resolve({ ...withEmoji, recalled_at: '2026-10-01T00:00:00Z' })?.kind).toBe('deleted')
  })

  test('the bubble draws the inline emoji with the meta spacer after it', () => {
    services.emoji.seed([['e1', emojiFixture('e1')]])
    const html = renderToStaticMarkup(
      <MessageBubble message={makeMessage({ content: 'ok 😺', entities: [entity(3, 2, 'e1')] })} ctx={makeCtx()} />,
    )
    expect(html).toContain('tg-entity-text')
    expect(html.indexOf('alt="😺"')).toBeLessThan(html.lastIndexOf('</div>'))
  })

  test('the registration can be undone (hot reload)', () => {
    expect(messageContent.kinds()).toContain('entity_text')
    expect(typeof unregisterEntityText).toBe('function')
  })
})

describe('EmojiStatus', () => {
  const status = (expires_at: string | null): EmojiStatusValue => ({
    user_id: 'u1',
    custom_emoji_id: 'e1',
    expires_at,
    emoji: emojiFixture('e1', '⭐'),
  })

  test('renders the status emoji next to a name and nothing without one', () => {
    services.emoji.seed([['e1', emojiFixture('e1', '⭐')]])
    services.statuses.seed([['u1', status(null)]])
    expect(renderToStaticMarkup(<EmojiStatus userId="u1" />)).toContain('tg-emoji-status')
    expect(renderToStaticMarkup(<EmojiStatus userId="nobody" />)).toBe('')
  })

  test('an expired status is hidden without waiting for a refetch', () => {
    services.statuses.seed([['u1', status(new Date(NOW - 1000).toISOString())]])
    expect(renderToStaticMarkup(<EmojiStatus userId="u1" />)).toBe('')
    services.statuses.seed([['u1', status(new Date(NOW + 60_000).toISOString())]])
    expect(renderToStaticMarkup(<EmojiStatus userId="u1" />)).toContain('aria-label="⭐"')
  })
})

describe('CustomEmojiGrid', () => {
  const set = (archived: boolean): CustomEmojiSet => ({
    id: archived ? 'old' : 'set-1',
    short_name: 'cats',
    title: archived ? 'Archived' : 'Cats',
    set_type: 'custom_emoji',
    installed: true,
    archived,
    stickers: [{ ...emojiFixture('e1'), emojis: ['😺'] }],
  })

  test('lists non-archived sets as buttons titled with their fallback emoji', () => {
    const html = renderToStaticMarkup(<CustomEmojiGrid sets={[set(false), set(true)]} onPick={() => {}} />)
    expect(html).toContain('aria-label="Cats"')
    expect(html).not.toContain('Archived')
    expect(html).toContain('title="😺"')
  })

  test('an empty library says so', () => {
    expect(renderToStaticMarkup(<CustomEmojiGrid sets={[]} onPick={() => {}} />)).toContain('还没有添加自定义表情包')
  })
})

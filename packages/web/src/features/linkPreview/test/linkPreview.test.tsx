import { describe, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { messageContent } from '../../message/content/messageContent'
import { makeMessage } from '../../message/fixtures/bubbleFixtures'
import { ComposerLinkPreview } from '../ComposerLinkPreview'
import { LinkPreviewContent } from '../LinkPreviewContent'
import {
  applyLinkPreviewFrame,
  dismissComposerLink,
  effectiveCard,
  linkPreviewStore,
  takeDismissal,
} from '../linkPreviewStore'
import '../register'

const card = { url: 'https://example.com/', site_name: 'Example', title: 'A title', description: 'Words' }

describe('TG-408 link previews (web)', () => {
  test('text with a link renders through the link kind; plain text does not', () => {
    expect(messageContent.resolve(makeMessage({ content: 'see https://example.com' }))?.kind).toBe('link')
    expect(messageContent.resolve(makeMessage({ content: 'no link' }))?.kind).toBe('text')
  })

  test('a frame card wins over the snapshot, and a null frame hides it', () => {
    expect(effectiveCard(card, undefined)).toBe(card)
    expect(effectiveCard(undefined, card)).toBe(card)
    expect(effectiveCard(card, null)).toBeNull()
    applyLinkPreviewFrame({ type: 'link_preview_updated', message_id: 'm1', preview: null })
    expect(linkPreviewStore.getState().cards.m1).toBeNull()
  })

  test('the card renders site, title and a safe link; without a card only the text shows', () => {
    const props = { ctx: {} as never, actions: {} as never, metaSpacer: null }
    const html = renderToStaticMarkup(
      <LinkPreviewContent
        message={makeMessage({ content: 'see https://example.com', link_preview: card })}
        {...props}
      />,
    )
    expect(html).toContain('Example')
    expect(html).toContain('A title')
    expect(html).toContain('rel="noopener noreferrer"')
    const bare = renderToStaticMarkup(
      <LinkPreviewContent message={makeMessage({ content: 'see https://x.org' })} {...props} />,
    )
    expect(bare).not.toContain('tg-link-card')
  })

  test('a composer dismissal applies to the next send of that link only', () => {
    dismissComposerLink('c1', 'https://example.com/a')
    expect(takeDismissal('c1', 'look https://example.com/a')).toBe(true)
    expect(takeDismissal('c1', 'look https://example.com/a')).toBe(false)
    dismissComposerLink('c1', 'https://example.com/a')
    expect(takeDismissal('c1', 'another https://other.org')).toBe(false)
    expect(renderToStaticMarkup(<ComposerLinkPreview chatId="c1" text="no link" />)).toBe('')
  })
})

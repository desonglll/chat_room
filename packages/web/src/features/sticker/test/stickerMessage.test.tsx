import { describe, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { storedMessageToBroadcast } from '@tg/core'
import { MessageBubble, messageContent } from '../../message'
import { makeCtx, makeMessage } from '../../message/fixtures/bubbleFixtures'
import { fitSticker, isStickerMessage, stickerMessageView } from '../message/stickerMessageModel'
import '../register'
import { makeSticker, storedSticker } from './fixtures'

const stickerMessage = () => storedMessageToBroadcast(storedSticker('m1', 'c1', makeSticker('st', ['🐱'])))

describe('sticker messages', () => {
  test('history keeps media_kind and the sticker block through the REST → WS bridge', () => {
    const message = stickerMessage()
    expect(message.media_kind).toBe('sticker')
    expect(message.sticker?.set_short_name).toBe('cats')
    const plain = storedMessageToBroadcast({
      ...storedSticker('m2', 'c1', makeSticker('x')),
      media_kind: undefined,
      sticker: undefined,
    } as never)
    expect('media_kind' in plain).toBe(false)
    expect('sticker' in plain).toBe(false)
  })

  test('recognised by media_kind, or by the TGS MIME when the fields were lost', () => {
    expect(isStickerMessage(stickerMessage())).toBe(true)
    const { media_kind: _kind, sticker: _sticker, ...forwarded } = stickerMessage()
    expect(isStickerMessage(forwarded)).toBe(true)
    expect(stickerMessageView(forwarded)?.format).toBe('tgs')
    expect(isStickerMessage(makeMessage())).toBe(false)
  })

  test('registered as a bare, overlay-meta kind that beats image but not the recalled placeholder', () => {
    const kind = messageContent.resolve(stickerMessage())
    expect(kind?.kind).toBe('sticker')
    expect(kind?.frame(stickerMessage())).toBe('bare')
    expect(kind?.metaPlacement(stickerMessage())).toBe('overlay')
    expect(messageContent.resolve({ ...stickerMessage(), recalled_at: '2026-10-01T00:00:00Z' })?.kind).toBe('deleted')
  })

  test('the bubble draws no fill and no tail around a sticker', () => {
    const html = renderToStaticMarkup(<MessageBubble message={stickerMessage()} ctx={makeCtx()} />)
    expect(html).toContain('data-frame="bare"')
    expect(html).not.toContain('tg-bubble__tail')
    expect(html).toContain('class="tg-sticker-message"')
    expect(html).toContain('aria-label="贴纸 🐱，查看贴纸包"')
    expect(html).toContain('data-overlay')
  })

  test('non-square stickers keep their aspect inside the box', () => {
    expect(fitSticker(512, 256)).toEqual({ width: 192, height: 96 })
    expect(fitSticker(0, 0)).toEqual({ width: 192, height: 192 })
  })
})

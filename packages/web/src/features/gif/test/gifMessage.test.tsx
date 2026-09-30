import { describe, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { storedMessageToBroadcast } from '@tg/core'
import { MessageBubble, messageContent } from '../../message'
import { buildMessageMenu } from '../../message/messageMenu'
import { makeCtx, makeMessage } from '../../message/fixtures/bubbleFixtures'
import { mediaPanelTabStore } from '../../sticker/panel/mediaPanelTabs'
import { isGifMessage } from '../gifModel'
import { buildGifSections } from '../panel/GifTab'
import '../register'
import { recentGif, savedGif, storedGif } from './fixtures'

const gifMessage = (mime = 'video/mp4') => storedMessageToBroadcast(storedGif('m1', 'c1', mime))

describe('GIF messages', () => {
  test('recognised by media_kind, or by a non-sensitive image/gif attachment', () => {
    expect(isGifMessage(gifMessage())).toBe(true)
    const plainGif = makeMessage({
      attachment: {
        id: 'a',
        file_name: 'x.gif',
        mime_type: 'image/gif',
        size_bytes: 1,
        download_url: '/a',
        is_sensitive: false,
      },
    })
    expect(isGifMessage(plainGif)).toBe(true)
    expect(isGifMessage({ ...plainGif, attachment: { ...plainGif.attachment!, is_sensitive: true } })).toBe(false)
    expect(isGifMessage(makeMessage())).toBe(false)
    expect(isGifMessage({ ...gifMessage(), recalled_at: '2026-10-01T00:00:00Z' })).toBe(false)
  })

  test('registered as a borderless overlay-meta kind above image/video, below the recalled placeholder', () => {
    const kind = messageContent.resolve(gifMessage())
    expect(kind?.kind).toBe('gif')
    expect(kind?.frame(gifMessage())).toBe('media')
    expect(kind?.metaPlacement(gifMessage())).toBe('overlay')
    expect(kind?.frame({ ...gifMessage(), content: 'caption' })).toBe('bubble')
    expect(messageContent.resolve({ ...gifMessage(), recalled_at: '2026-10-01T00:00:00Z' })?.kind).toBe('deleted')
  })

  test('the bubble renders a muted looping inline video, still (not autoplaying) until mounted', () => {
    const html = renderToStaticMarkup(<MessageBubble message={gifMessage()} ctx={makeCtx()} />)
    expect(html).toContain('<video')
    expect(html).toContain('muted')
    expect(html).toContain('loop')
    expect(html).toMatch(/playsinline/i)
    expect(html).not.toContain('autoplay')
    expect(html).toContain('tg-gif__badge')
    // A real GIF is a still canvas until it plays: no <img> means no decoding.
    const still = renderToStaticMarkup(<MessageBubble message={gifMessage('image/gif')} ctx={makeCtx()} />)
    expect(still).toContain('<canvas')
    expect(still).not.toContain('<img')
  })

  test('"保存 GIF" is in the menu of a delivered GIF message only', () => {
    const ids = (message: ReturnType<typeof gifMessage>, delivered = true) =>
      buildMessageMenu(message, {}, { delivered }).map((item) => item.id)
    expect(ids(gifMessage())).toContain('save-gif')
    expect(ids(gifMessage(), false)).not.toContain('save-gif')
    expect(ids(makeMessage())).not.toContain('save-gif')
  })

  test('the GIF tab replaces the panel placeholder', () => {
    const tab = mediaPanelTabStore.getState().tabs.find((entry) => entry.id === 'gif')
    expect(tab?.label).toBe('GIF')
    expect(tab?.order).toBe(30)
  })

  test('panel sections: saved first, geometry from the server or the measured media', () => {
    const sections = buildGifSections([savedGif('g1')], [recentGif('m9')], { '/api/attachments/a-m9?key=k': 2 })
    expect(sections.map((section) => section.items.map((item) => [item.key, item.aspect]))).toEqual([
      [['g1', 480 / 270]],
      [['m9', 2]],
    ])
  })
})

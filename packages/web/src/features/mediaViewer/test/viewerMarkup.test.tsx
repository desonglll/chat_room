/**
 * Markup over the viewer's server-renderable parts (`bun test` has no DOM; the portal-owning
 * `ViewerSurface` is exercised in the browser, see docs/devlog/TG-105.md "Verification").
 */
import { describe, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { MediaViewer } from '../MediaViewer'
import { MediaSlide } from '../MediaSlide'
import { ThumbStrip, STRIP_RADIUS } from '../ThumbStrip'
import { Caption, NavArrow, TopBar, formatWhen } from '../ViewerChrome'
import { createMediaViewerStore, mediaViewerStore, openMediaViewer, closeMediaViewer } from '../mediaViewerStore'
import { media } from './fixtures'

const count = (html: string, needle: string) => html.split(needle).length - 1
const noop = () => {}

describe('store', () => {
  test('open carries the request, a new open gets a new id, close clears', () => {
    const store = createMediaViewerStore()
    store.getState().open({ chatId: 'c1', attachmentId: 'a1' })
    const first = store.getState().request
    expect(first).toMatchObject({ chatId: 'c1', attachmentId: 'a1', sourceRect: null })
    store.getState().open({ chatId: 'c1', attachmentId: 'a1', sourceRect: { x: 1, y: 2, width: 3, height: 4 } })
    expect(store.getState().request?.openId).toBeGreaterThan(first?.openId ?? 0)
    expect(store.getState().request?.sourceRect).toEqual({ x: 1, y: 2, width: 3, height: 4 })
    store.getState().close()
    expect(store.getState().request).toBeNull()
  })

  test('the app-wide helpers drive the singleton', () => {
    openMediaViewer({ chatId: 'c', attachmentId: 'a' })
    expect(mediaViewerStore.getState().request?.attachmentId).toBe('a')
    closeMediaViewer()
    expect(mediaViewerStore.getState().request).toBeNull()
  })
})

test('MediaViewer renders nothing while closed', () => {
  expect(renderToStaticMarkup(<MediaViewer />)).toBe('')
})

describe('top bar', () => {
  const item = media(7, { sender: 'bob', fileName: 'cat.jpg' })

  test('always offers download and close; forward/delete only when the host wires them', () => {
    const html = renderToStaticMarkup(
      <TopBar item={item} position="" actions={{}} deleting={false} onForward={noop} onDelete={noop} onClose={noop} />,
    )
    expect(html).toContain('data-mv-chrome')
    expect(html).toContain('bob')
    expect(html).toContain(formatWhen(item.createdAt))
    expect(html).toContain(`href="${item.url.replace('&', '&amp;')}"`)
    expect(html).toContain('download="cat.jpg"')
    expect(html).toContain('aria-label="下载"')
    expect(html).toContain('aria-label="关闭"')
    expect(html).not.toContain('aria-label="转发"')
    expect(html).not.toContain('aria-label="删除"')
  })

  test('forward, delete, and canDelete gating; position label; busy delete is disabled', () => {
    const actions = { onForward: noop, onDelete: () => true }
    const html = renderToStaticMarkup(
      <TopBar
        item={item}
        position="3 / 9"
        actions={actions}
        deleting={true}
        onForward={noop}
        onDelete={noop}
        onClose={noop}
      />,
    )
    expect(html).toContain('aria-label="转发"')
    expect(html).toMatch(/aria-label="删除"[^>]*disabled=""|disabled=""[^>]*aria-label="删除"/)
    expect(html).toContain('3 / 9')
    const denied = renderToStaticMarkup(
      <TopBar
        item={item}
        position=""
        actions={{ ...actions, canDelete: () => false }}
        deleting={false}
        onForward={noop}
        onDelete={noop}
        onClose={noop}
      />,
    )
    expect(denied).not.toContain('aria-label="删除"')
  })
})

test('arrows are named by direction; caption renders only when there is text', () => {
  expect(renderToStaticMarkup(<NavArrow direction="left" onClick={noop} />)).toContain('aria-label="上一个"')
  expect(renderToStaticMarkup(<NavArrow direction="right" onClick={noop} />)).toContain('aria-label="下一个"')
  expect(renderToStaticMarkup(<Caption item={media(1, { caption: '看 https://example.com' })} />)).toContain(
    'href="https://example.com"',
  )
  expect(renderToStaticMarkup(<Caption item={media(1, { caption: '  ' })} />)).toBe('')
  expect(renderToStaticMarkup(<Caption item={media(1, { caption: null })} />)).toBe('')
})

describe('slides', () => {
  test('an image slide is a plain, non-draggable img', () => {
    const html = renderToStaticMarkup(<MediaSlide item={media(1)} active revealed onReveal={noop} />)
    expect(html).toContain('class="tg-mv__media"')
    expect(html).toContain('draggable="false"')
    expect(html).toContain('alt="photo-1.jpg"')
  })

  test('a sensitive medium stays veiled until revealed', () => {
    const item = media(2, { isSensitive: true })
    const veiled = renderToStaticMarkup(<MediaSlide item={item} active revealed={false} onReveal={noop} />)
    expect(veiled).toContain('tg-mv__veil')
    expect(veiled).toContain('敏感内容')
    expect(veiled).toContain('tg-mv__media--veiled')
    const shown = renderToStaticMarkup(<MediaSlide item={item} active revealed onReveal={noop} />)
    expect(shown).not.toContain('敏感内容')
  })

  test('the active video is a player host sized to its natural ratio; a neighbour is a still', () => {
    const video = media(3, { kind: 'video', mimeType: 'video/mp4' })
    const active = renderToStaticMarkup(
      <MediaSlide item={video} active revealed onReveal={noop} natural={{ width: 1280, height: 720 }} />,
    )
    expect(active).toContain('data-mv-video')
    expect(active).toContain('--mv-w:1280')
    expect(active).toContain('--mv-h:720')
    const still = renderToStaticMarkup(<MediaSlide item={video} active={false} revealed onReveal={noop} />)
    expect(still).toContain('tg-mv__still')
    expect(still).toContain('preload="metadata"')
    expect(still).not.toContain('data-mv-video')
  })
})

describe('thumbnail strip', () => {
  test('marks the current item and names each thumbnail by kind', () => {
    const items = [media(1), media(2, { kind: 'video' }), media(3)]
    const html = renderToStaticMarkup(<ThumbStrip items={items} index={1} onSelect={noop} reduced={false} />)
    expect(count(html, 'class="tg-mv__thumb"')).toBe(3)
    expect(count(html, 'aria-current="true"')).toBe(1)
    expect(html).toMatch(/aria-current="true"[^>]*aria-label="视频 photo-2.jpg"/)
    expect(html).toContain('aria-label="图片 photo-1.jpg"')
  })

  test('is windowed around the current item and hidden for a single medium', () => {
    const items = Array.from({ length: 500 }, (_, i) => media(i + 1))
    const html = renderToStaticMarkup(<ThumbStrip items={items} index={250} onSelect={noop} reduced />)
    expect(count(html, 'class="tg-mv__thumb"')).toBe(STRIP_RADIUS * 2 + 1)
    expect(renderToStaticMarkup(<ThumbStrip items={[media(1)]} index={0} onSelect={noop} reduced />)).toBe('')
  })
})

describe('TG-1302 thumbnail first', () => {
  const thumb = '/api/attachments/a1/thumbnail?key=k1'
  test('the stage paints the thumbnail under the still-loading original', () => {
    const html = renderToStaticMarkup(
      <MediaSlide item={media(1, { previewUrl: thumb })} active revealed={false} onReveal={noop} />,
    )
    expect(html).toContain(`class="tg-mv__media tg-mv__media--preview" src="${thumb.replace('&', '&amp;')}"`)
    expect(html).toContain('data-loading=""')
    expect(html).toContain(`src="${media(1).url}"`)
  })

  test('without a thumbnail the original is the only image, shown at once', () => {
    const html = renderToStaticMarkup(<MediaSlide item={media(1)} active revealed={false} onReveal={noop} />)
    expect(count(html, '<img')).toBe(1)
    expect(html).not.toContain('data-loading')
  })

  test('the strip shows thumbnails', () => {
    const html = renderToStaticMarkup(
      <ThumbStrip items={[media(1, { previewUrl: thumb }), media(2)]} index={0} onSelect={noop} reduced />,
    )
    expect(html).toContain(`src="${thumb}"`)
    expect(html).not.toContain(`src="${media(1).url}"`)
  })
})

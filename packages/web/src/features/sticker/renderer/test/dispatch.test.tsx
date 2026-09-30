/** `<Sticker>` picks the renderer from the wire format (server markup; no DOM in bun). */
import { describe, expect, test } from 'bun:test'
import { gzipSync } from 'node:zlib'
import { renderToStaticMarkup } from 'react-dom/server'
import { Sticker } from '../index'
import { harness } from './managerHarness'

const render = (element: React.ReactElement) => renderToStaticMarkup(element)
const EBML = new Uint8Array([0x1a, 0x45, 0xdf, 0xa3, 0x9f, 0x42, 0x86, 0x81, 0x01])

describe('<Sticker> dispatch', () => {
  test('tgs → the Lottie canvas renderer', () => {
    const html = render(<Sticker sticker={{ format: 'tgs', file_url: '/f/1', emoji: '🐱' }} size={96} />)
    expect(html).toContain('<canvas class="tg-sticker__canvas"')
    expect(html).toContain('aria-label="🐱"')
    expect(html).not.toContain('data-format')
  })

  test('webp → a plain image, loading until decoded', () => {
    const html = render(<Sticker sticker={{ format: 'webp', file_url: '/f/2', emoji: '🐶' }} size={96} />)
    expect(html).toContain('data-format="webp"')
    expect(html).toContain('data-phase="loading"')
    expect(html).toMatch(/<img class="tg-sticker__still" src="\/f\/2"/)
    expect(html).not.toContain('<canvas')
  })

  test('webm → a muted, looping, inline video', () => {
    const html = render(
      <Sticker sticker={{ format: 'webm', download_url: '/a/3', emoji: '🦊' }} webmSupported size={96} />,
    )
    expect(html).toContain('data-format="webm"')
    expect(html).toMatch(/<video class="tg-sticker__video" src="\/a\/3"[^>]* loop=""[^>]* playsInline=""/)
    expect(html).toContain('aria-hidden="true"')
  })

  test('mime_type alone is enough (message attachment without a format)', () => {
    const html = render(<Sticker sticker={{ mime_type: 'image/webp', download_url: '/a/4' }} />)
    expect(html).toContain('data-format="webp"')
  })

  test('no WebM alpha support → the thumbnail, never the video', () => {
    const html = render(
      <Sticker
        sticker={{ format: 'webm', file_url: '/f/5', thumbnail_url: '/t/5.webp', emoji: '🦊' }}
        webmSupported={false}
      />,
    )
    expect(html).toContain('data-fallback="unsupported"')
    expect(html).toContain('data-phase="static"')
    expect(html).toContain('<img class="tg-sticker__still" src="/t/5.webp"')
    expect(html).not.toContain('<video')
  })

  test('no WebM alpha support and no thumbnail → the emoji', () => {
    const html = render(<Sticker sticker={{ format: 'webm', file_url: '/f/6', emoji: '🦊' }} webmSupported={false} />)
    expect(html).toContain('<span class="tg-sticker__emoji" aria-hidden="true" style="font-size:96px">🦊</span>')
  })

  test('undeclared format: bytes are sniffed synchronously', () => {
    const tgs = render(<Sticker sticker={{ emoji: '🦊' }} data={new Uint8Array(gzipSync('{"v":"5"}'))} />)
    expect(tgs).toContain('<canvas class="tg-sticker__canvas"')
    // WebM/WebP bytes become a blob URL in an effect, so markup shows the loading box first.
    const webm = render(<Sticker sticker={{ emoji: '🦊' }} data={EBML} />)
    expect(webm).toContain('data-phase="loading"')
    expect(webm).not.toContain('data-phase="error"')
  })

  test('undeclared format behind a URL: a loading box until the bytes are sniffed', () => {
    const html = render(<Sticker sticker={{ download_url: '/a/7' }} />)
    expect(html).toContain('data-phase="loading"')
    expect(html).not.toContain('data-format')
  })

  test('unrecognizable bytes → the error box', () => {
    const html = render(<Sticker sticker={{}} data={new Uint8Array([1, 2, 3])} />)
    expect(html).toContain('data-phase="error"')
  })

  test('markup never registers views with the manager', () => {
    const h = harness()
    render(<Sticker sticker={{ format: 'webm', file_url: '/f/8' }} webmSupported manager={h.manager} />)
    expect(h.manager.stats().motionViews).toBe(0)
  })
})

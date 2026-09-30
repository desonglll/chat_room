/** Server markup of `<AnimatedSticker>` (bun has no DOM; the live path is covered by bench/). */
import { describe, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { AnimatedSticker } from '../index'
import { harness } from './managerHarness'

describe('AnimatedSticker markup', () => {
  test('before any frame: a sized box, the loading phase, a canvas held hidden', () => {
    const html = renderToStaticMarkup(<AnimatedSticker src="/s/1.tgs" size={128} label="🐱" />)
    expect(html).toContain('class="tg-sticker"')
    expect(html).toContain('data-phase="loading"')
    expect(html).toContain('role="img"')
    expect(html).toContain('aria-label="🐱"')
    expect(html).toContain('width:128px;height:128px')
    expect(html).toMatch(/<canvas class="tg-sticker__canvas" hidden="">/)
    expect(html).not.toContain('<img')
  })

  test('a server thumbnail is shown as the still until the first frame exists', () => {
    const html = renderToStaticMarkup(<AnimatedSticker data={new Uint8Array([1])} poster="/thumb.webp" />)
    expect(html).toContain('<img class="tg-sticker__still" src="/thumb.webp" alt="" draggable="false"/>')
  })

  test('rendering markup never touches the manager (no DOM on the server)', () => {
    const h = harness()
    renderToStaticMarkup(<AnimatedSticker src="/s/1.tgs" manager={h.manager} />)
    expect(h.manager.stats().views).toBe(0)
  })
})

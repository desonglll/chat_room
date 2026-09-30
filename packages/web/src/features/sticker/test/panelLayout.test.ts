import { describe, expect, test } from 'bun:test'
import { columnsFor, layoutPanel, sectionAt, sectionTop, visibleItems } from '../panel/panelLayout'
import { makeSticker } from './fixtures'

const stickers = (prefix: string, count: number) =>
  Array.from({ length: count }, (_, index) => makeSticker(`${prefix}${index}`))
const metrics = { headerHeight: 30, cellSize: 70, sectionGap: 10 }

describe('panel layout', () => {
  test('headers and rows stack with fixed heights; empty sections vanish', () => {
    const layout = layoutPanel(
      [
        { id: 'favorites', title: '收藏', stickers: [] },
        { id: 'a', title: 'A', stickers: stickers('a', 9) },
        { id: 'b', title: 'B', stickers: stickers('b', 4) },
      ],
      4,
      metrics,
    )
    expect(layout.items.map((item) => `${item.kind}@${item.top}`)).toEqual([
      'header@0',
      'row@30',
      'row@100',
      'row@170',
      'header@250',
      'row@280',
    ])
    expect(layout.height).toBe(360)
    expect(layout.sectionTops).toEqual([
      { id: 'a', top: 0 },
      { id: 'b', top: 250 },
    ])
    expect(sectionTop(layout, 'b')).toBe(250)
    expect(sectionTop(layout, 'favorites')).toBeNull()
    const untitled = layoutPanel([{ id: 'only', title: '', stickers: stickers('o', 4) }], 4, metrics)
    expect(untitled.items.map((item) => `${item.kind}@${item.top}`)).toEqual(['row@0'])
  })

  test('300 stickers mount only the rows near the viewport', () => {
    const sections = Array.from({ length: 12 }, (_, index) => ({
      id: `s${index}`,
      title: `S${index}`,
      stickers: stickers(`s${index}-`, 25),
    }))
    const layout = layoutPanel(sections, columnsFor(300, 72))
    expect(layout.columns).toBe(4)
    const visible = visibleItems(layout, 0, 320, 144)
    const mounted = visible.flatMap((item) => (item.kind === 'row' ? item.stickers : []))
    expect(mounted.length).toBeLessThanOrEqual(4 * 8)
    expect(mounted.length).toBeGreaterThan(0)
    const deep = visibleItems(layout, 5000, 320, 0)
    expect(deep.every((item) => item.top + item.height > 5000 && item.top < 5320)).toBe(true)
  })

  test('the active section is the last header at or above the scroll position', () => {
    const layout = layoutPanel(
      [
        { id: 'a', title: 'A', stickers: stickers('a', 8) },
        { id: 'b', title: 'B', stickers: stickers('b', 8) },
      ],
      4,
      metrics,
    )
    expect(sectionAt(layout, 0)).toBe('a')
    expect(sectionAt(layout, 178)).toBe('a')
    expect(sectionAt(layout, 179.5)).toBe('b') // sub-pixel scrollTo lands within 1px
    expect(sectionAt(layout, 180)).toBe('b')
    expect(sectionAt(layoutPanel([], 4), 0)).toBeNull()
  })
})

import { describe, expect, test } from 'bun:test'
import { clampAspect, masonryLayout, visibleBoxes } from '../panel/masonry'
import { GIF_GAP, gifPanelLayout, HEADER_HEIGHT } from '../panel/gifPanelLayout'

describe('masonryLayout', () => {
  test('fits as many columns as the minimum width allows, at least two', () => {
    expect(masonryLayout([], { width: 340, gap: 4, minColumnWidth: 100 }).columns).toBe(3)
    expect(masonryLayout([], { width: 150, gap: 4, minColumnWidth: 100 }).columns).toBe(2)
    expect(masonryLayout([], { width: 0, gap: 4, minColumnWidth: 100 }).height).toBe(0)
  })

  test('places each item in the currently shortest column at its own aspect ratio', () => {
    // Two 100px columns (204 = 100 + 4 + 100).
    const layout = masonryLayout([{ aspect: 1 }, { aspect: 2 }, { aspect: 0.5 }, { aspect: 1 }], {
      width: 204,
      gap: 4,
      minColumnWidth: 100,
    })
    expect(layout.columns).toBe(2)
    expect(layout.boxes).toEqual([
      { x: 0, y: 0, width: 100, height: 100 },
      { x: 104, y: 0, width: 100, height: 50 },
      // Column 2 (height 54) is shorter than column 1 (104).
      { x: 104, y: 54, width: 100, height: 200 },
      { x: 0, y: 104, width: 100, height: 100 },
    ])
    expect(layout.height).toBe(254)
  })

  test('the tie goes to the leftmost column, so a row of squares fills left to right', () => {
    const layout = masonryLayout([{ aspect: 1 }, { aspect: 1 }, { aspect: 1 }], {
      width: 308,
      gap: 4,
      minColumnWidth: 100,
    })
    expect(layout.boxes.map((box) => box.x)).toEqual([0, 104, 208])
    expect(layout.boxes.every((box) => box.y === 0)).toBe(true)
  })

  test('unknown or extreme geometry is clamped', () => {
    expect(clampAspect(null)).toBe(1)
    expect(clampAspect(Number.NaN)).toBe(1)
    expect(clampAspect(0)).toBe(1)
    expect(clampAspect(10)).toBe(2.5)
    expect(clampAspect(0.1)).toBe(0.5)
    const layout = masonryLayout([{ aspect: 100 }], { width: 204, gap: 4, minColumnWidth: 100 })
    expect(layout.boxes[0]?.height).toBe(40)
  })

  test('visibleBoxes keeps only boxes near the viewport', () => {
    const boxes = [0, 200, 400, 600, 800].map((y) => ({ x: 0, y, width: 100, height: 100 }))
    expect(visibleBoxes(boxes, 250, 200, 0)).toEqual([1, 2])
    expect(visibleBoxes(boxes, 250, 200, 400)).toEqual([0, 1, 2, 3, 4])
  })
})

describe('gifPanelLayout', () => {
  test('stacks a header and a masonry per non-empty section', () => {
    const layout = gifPanelLayout(
      [
        { id: 'saved', title: 'Saved', items: [{ key: 'a', aspect: 1 }] },
        { id: 'empty', title: 'Empty', items: [] },
        { id: 'recent', title: 'Recent', items: [{ key: 'b', aspect: 2 }] },
      ],
      204,
    )
    expect(layout.headers.map((header) => [header.id, header.top])).toEqual([
      ['saved', 0],
      ['recent', HEADER_HEIGHT + 100 + GIF_GAP],
    ])
    expect(layout.cells.map((cell) => [cell.item.key, cell.box.y])).toEqual([
      ['a', HEADER_HEIGHT],
      ['b', HEADER_HEIGHT * 2 + 100 + GIF_GAP],
    ])
    expect(layout.height).toBe(HEADER_HEIGHT * 2 + 100 + 50 + GIF_GAP * 2)
  })
})

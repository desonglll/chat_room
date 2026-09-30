import { describe, expect, test } from 'bun:test'
import { layoutAlbum, type AlbumItemSize, type AlbumLayout } from './albumLayout'

const W = 420
const options = { maxWidth: W, minWidth: 100, spacing: 2 }
const square: AlbumItemSize = { width: 1000, height: 1000 }
const wide: AlbumItemSize = { width: 1600, height: 900 }
const narrow: AlbumItemSize = { width: 900, height: 1600 }
const panorama: AlbumItemSize = { width: 4000, height: 800 }
const tower: AlbumItemSize = { width: 400, height: 3000 }

function overlaps(a: AlbumLayout['tiles'][number], b: AlbumLayout['tiles'][number]): boolean {
  const epsilon = 0.5
  return (
    a.x < b.x + b.width - epsilon &&
    b.x < a.x + a.width - epsilon &&
    a.y < b.y + b.height - epsilon &&
    b.y < a.y + a.height - epsilon
  )
}

/** Invariants every layout must satisfy, whatever the input. */
function assertSound(layout: AlbumLayout, count: number) {
  expect(layout.tiles).toHaveLength(Math.min(count, 10))
  for (const tile of layout.tiles) {
    expect(tile.width).toBeGreaterThan(0)
    expect(tile.height).toBeGreaterThan(0)
    expect(tile.x).toBeGreaterThanOrEqual(0)
    expect(tile.y).toBeGreaterThanOrEqual(0)
    expect(tile.x + tile.width).toBeLessThanOrEqual(W + 0.5)
  }
  for (let i = 0; i < layout.tiles.length; i += 1) {
    for (let j = i + 1; j < layout.tiles.length; j += 1) {
      expect(overlaps(layout.tiles[i]!, layout.tiles[j]!)).toBe(false)
    }
  }
  // Every outer edge is touched by at least one tile flagged for it (the bubble corners).
  expect(layout.tiles.some((tile) => tile.sides.top && tile.sides.left)).toBe(true)
  expect(layout.tiles.some((tile) => tile.sides.top && tile.sides.right)).toBe(true)
  expect(layout.tiles.some((tile) => tile.sides.bottom && tile.sides.left)).toBe(true)
  expect(layout.tiles.some((tile) => tile.sides.bottom && tile.sides.right)).toBe(true)
}

function rows(layout: AlbumLayout): number[][] {
  const byY = new Map<number, number[]>()
  layout.tiles.forEach((tile, index) => {
    const key = Math.round(tile.y)
    byY.set(key, [...(byY.get(key) ?? []), index])
  })
  return [...byY.entries()].sort(([a], [b]) => a - b).map(([, indices]) => indices)
}

describe('layoutAlbum', () => {
  test('empty and single inputs', () => {
    expect(layoutAlbum([], options).tiles).toEqual([])
    const one = layoutAlbum([wide], options)
    expect(one.tiles).toHaveLength(1)
    expect(one.tiles[0]!.sides).toEqual({ top: true, bottom: true, left: true, right: true })
  })

  test('two wide items of the same shape stack; two squares sit side by side', () => {
    const stacked = layoutAlbum([wide, wide], options)
    assertSound(stacked, 2)
    expect(rows(stacked)).toEqual([[0], [1]])
    expect(stacked.tiles[0]!.width).toBe(W)

    const sideBySide = layoutAlbum([square, square], options)
    assertSound(sideBySide, 2)
    expect(rows(sideBySide)).toEqual([[0, 1]])
    expect(sideBySide.tiles[0]!.width).toBeCloseTo(sideBySide.tiles[1]!.width, 5)
  })

  test('two mixed items share a row with widths following their ratios', () => {
    const layout = layoutAlbum([narrow, wide], options)
    assertSound(layout, 2)
    expect(rows(layout)).toEqual([[0, 1]])
    expect(layout.tiles[0]!.width + layout.tiles[1]!.width + 2).toBeCloseTo(W, 5)
  })

  test('three: a narrow first item becomes a full-height left column', () => {
    const layout = layoutAlbum([narrow, square, square], options)
    assertSound(layout, 3)
    const [left, topRight, bottomRight] = layout.tiles as [
      AlbumLayout['tiles'][number],
      AlbumLayout['tiles'][number],
      AlbumLayout['tiles'][number],
    ]
    expect(left.x).toBe(0)
    expect(left.height).toBeCloseTo(topRight.height + bottomRight.height + 2, 5)
    expect(topRight.x).toBe(bottomRight.x)
  })

  test('three: otherwise one full-width item on top and two below', () => {
    const layout = layoutAlbum([wide, square, square], options)
    assertSound(layout, 3)
    expect(rows(layout)).toEqual([[0], [1, 2]])
    expect(layout.tiles[0]!.width).toBe(W)
  })

  test('four: wide first on top of three; otherwise a left column and three stacked', () => {
    const top = layoutAlbum([wide, square, square, square], options)
    assertSound(top, 4)
    expect(rows(top)).toEqual([[0], [1, 2, 3]])

    const column = layoutAlbum([narrow, square, square, square], options)
    assertSound(column, 4)
    expect(column.tiles[0]!.x).toBe(0)
    expect(new Set(column.tiles.slice(1).map((tile) => tile.x)).size).toBe(1)
  })

  test('five to ten items pack into 2–4 full-width rows of at most three', () => {
    for (let count = 5; count <= 10; count += 1) {
      const sizes = Array.from({ length: count }, (_, index) => [square, wide, narrow][index % 3]!)
      const layout = layoutAlbum(sizes, options)
      assertSound(layout, count)
      const lines = rows(layout)
      expect(lines.length).toBeGreaterThanOrEqual(2)
      expect(lines.length).toBeLessThanOrEqual(4)
      for (const line of lines) {
        expect(line.length).toBeLessThanOrEqual(4)
        const last = layout.tiles[line[line.length - 1]!]!
        expect(last.x + last.width).toBeCloseTo(W, 5)
      }
      // Items keep album order: reading row by row gives 0..count-1.
      expect(lines.flat()).toEqual(Array.from({ length: count }, (_, index) => index))
    }
  })

  test('extreme ratios never produce slivers or overflow', () => {
    for (const sizes of [
      [panorama, tower],
      [panorama, panorama, tower],
      [tower, tower, tower, tower],
      [panorama, square, tower, wide, narrow, panorama],
      [{ width: 0, height: 0 }, square],
    ]) {
      const layout = layoutAlbum(sizes, options)
      assertSound(layout, sizes.length)
      for (const tile of layout.tiles) expect(tile.width).toBeGreaterThanOrEqual(20)
    }
  })

  test('more than ten items are capped at ten', () => {
    const layout = layoutAlbum(
      Array.from({ length: 13 }, () => square),
      options,
    )
    assertSound(layout, 10)
  })
})

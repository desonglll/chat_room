/**
 * TG-403: Telegram's grouped-media ("album") mosaic, as a pure function.
 *
 * A port of Telegram Desktop's `LayoutMediaGroup` (ui/grouped_layout.cpp), the same algorithm
 * Telegram Web and the mobile apps use. It is NOT an equal grid: items are classified as
 * wide / narrow / square by aspect ratio, 2–4 items get hand-tuned arrangements, and 5–10
 * items are packed into 2–4 rows by trying every row split and keeping the one whose total
 * height is closest to the target while every row stays tall enough.
 *
 * Output coordinates are in the same units as `maxWidth` (CSS px), origin top-left.
 */

export interface AlbumItemSize {
  width: number
  height: number
}

/** Which outer edges of the bubble a tile touches — they get the bubble's corner radius. */
export interface AlbumTileSides {
  top: boolean
  bottom: boolean
  left: boolean
  right: boolean
}

export interface AlbumTile {
  x: number
  y: number
  width: number
  height: number
  sides: AlbumTileSides
}

export interface AlbumLayout {
  width: number
  height: number
  tiles: AlbumTile[]
}

export interface AlbumLayoutOptions {
  maxWidth: number
  minWidth: number
  /** Gap between tiles. */
  spacing: number
  /** Telegram: maxWidth / maxHeight = 1.0 for albums. */
  maxHeight?: number
}

export const DEFAULT_ALBUM_LAYOUT: AlbumLayoutOptions = { maxWidth: 420, minWidth: 100, spacing: 2 }

const SIDE_TOP = 1
const SIDE_BOTTOM = 2
const SIDE_LEFT = 4
const SIDE_RIGHT = 8

interface Rect {
  x: number
  y: number
  w: number
  h: number
  sides: number
}

function ratioOf(size: AlbumItemSize): number {
  // A missing / zero dimension is treated as square rather than dividing by zero.
  if (!(size.width > 0) || !(size.height > 0)) return 1
  return size.width / size.height
}

function proportionOf(ratio: number): 'w' | 'n' | 'q' {
  if (ratio > 1.2) return 'w'
  if (ratio < 0.8) return 'n'
  return 'q'
}

function toTile(rect: Rect): AlbumTile {
  return {
    x: rect.x,
    y: rect.y,
    width: rect.w,
    height: rect.h,
    sides: {
      top: (rect.sides & SIDE_TOP) !== 0,
      bottom: (rect.sides & SIDE_BOTTOM) !== 0,
      left: (rect.sides & SIDE_LEFT) !== 0,
      right: (rect.sides & SIDE_RIGHT) !== 0,
    },
  }
}

function finish(rects: Rect[]): AlbumLayout {
  const width = Math.max(0, ...rects.map((rect) => rect.x + rect.w))
  const height = Math.max(0, ...rects.map((rect) => rect.y + rect.h))
  return { width, height, tiles: rects.map(toTile) }
}

/** Lay out 1–10 items; more than 10 are laid out as the first 10 (Telegram's album limit). */
export function layoutAlbum(
  sizes: readonly AlbumItemSize[],
  options: AlbumLayoutOptions = DEFAULT_ALBUM_LAYOUT,
): AlbumLayout {
  const items = sizes.slice(0, 10)
  const count = items.length
  if (count === 0) return { width: 0, height: 0, tiles: [] }
  const maxWidth = options.maxWidth
  const minWidth = options.minWidth
  const maxHeight = options.maxHeight ?? maxWidth
  const maxRatio = maxWidth / maxHeight
  const ratios = items.map(ratioOf)
  const proportions = ratios.map(proportionOf).join('')
  const averageRatio = ratios.reduce((sum, ratio) => sum + ratio, 0) / count

  if (count === 1) {
    const ratio = ratios[0]!
    const width = Math.min(maxWidth, Math.max(minWidth, maxHeight * ratio))
    const height = Math.min(maxHeight, width / ratio)
    return finish([{ x: 0, y: 0, w: width, h: height, sides: SIDE_TOP | SIDE_BOTTOM | SIDE_LEFT | SIDE_RIGHT }])
  }
  if (count >= 5 || ratios.some((ratio) => ratio > 2)) {
    return finish(layoutComplex(ratios, averageRatio, options))
  }
  if (count === 2) return finish(layoutTwo(ratios, proportions, averageRatio, maxRatio, options, maxHeight))
  if (count === 3) return finish(layoutThree(ratios, proportions, options, maxHeight))
  return finish(layoutFour(ratios, proportions, options, maxHeight))
}

function layoutTwo(
  ratios: number[],
  proportions: string,
  averageRatio: number,
  maxRatio: number,
  { maxWidth, minWidth, spacing }: AlbumLayoutOptions,
  maxHeight: number,
): Rect[] {
  const [first, second] = ratios as [number, number]
  if (proportions === 'ww' && averageRatio > 1.4 * maxRatio && second - first < 0.2) {
    // Two wide items of similar shape: stacked.
    const width = maxWidth
    const height = Math.round(Math.min(width / first, width / second, (maxHeight - spacing) / 2))
    return [
      { x: 0, y: 0, w: width, h: height, sides: SIDE_LEFT | SIDE_TOP | SIDE_RIGHT },
      { x: 0, y: height + spacing, w: width, h: height, sides: SIDE_LEFT | SIDE_BOTTOM | SIDE_RIGHT },
    ]
  }
  if (proportions === 'ww' || proportions === 'qq') {
    // Side by side, equal widths.
    const width = (maxWidth - spacing) / 2
    const height = Math.round(Math.min(width / first, width / second, maxHeight))
    return [
      { x: 0, y: 0, w: width, h: height, sides: SIDE_TOP | SIDE_LEFT | SIDE_BOTTOM },
      { x: width + spacing, y: 0, w: width, h: height, sides: SIDE_TOP | SIDE_RIGHT | SIDE_BOTTOM },
    ]
  }
  // Side by side, widths proportional to the ratios.
  const minimal = Math.round(Math.min(minWidth * 1.5, maxWidth * 0.4))
  const secondWidth = Math.max(
    minimal,
    Math.round(Math.min(maxWidth * 0.5, (maxWidth - spacing) / (1 + second / first))),
  )
  const firstWidth = maxWidth - secondWidth - spacing
  const height = Math.min(maxHeight, Math.round(Math.min(firstWidth / first, secondWidth / second)))
  return [
    { x: 0, y: 0, w: firstWidth, h: height, sides: SIDE_TOP | SIDE_LEFT | SIDE_BOTTOM },
    { x: firstWidth + spacing, y: 0, w: secondWidth, h: height, sides: SIDE_TOP | SIDE_RIGHT | SIDE_BOTTOM },
  ]
}

function layoutThree(
  ratios: number[],
  proportions: string,
  { maxWidth, minWidth, spacing }: AlbumLayoutOptions,
  maxHeight: number,
): Rect[] {
  const [first, second, third] = ratios as [number, number, number]
  if (proportions[0] === 'n') {
    // A narrow first item as a full-height left column; two stacked on the right.
    const firstHeight = maxHeight
    const thirdHeight = Math.round(
      Math.min((maxHeight - spacing) / 2, (second * (maxWidth - spacing)) / (third + second)),
    )
    const secondHeight = firstHeight - thirdHeight - spacing
    const rightWidth = Math.max(
      minWidth,
      Math.round(Math.min((maxWidth - spacing) * 0.5, Math.min(thirdHeight * third, secondHeight * second))),
    )
    const leftWidth = Math.min(Math.round(firstHeight * first), maxWidth - spacing - rightWidth)
    return [
      { x: 0, y: 0, w: leftWidth, h: firstHeight, sides: SIDE_TOP | SIDE_LEFT | SIDE_BOTTOM },
      { x: leftWidth + spacing, y: 0, w: rightWidth, h: secondHeight, sides: SIDE_TOP | SIDE_RIGHT },
      {
        x: leftWidth + spacing,
        y: secondHeight + spacing,
        w: rightWidth,
        h: thirdHeight,
        sides: SIDE_BOTTOM | SIDE_RIGHT,
      },
    ]
  }
  // One full-width item on top, two side by side below.
  const width = maxWidth
  const firstHeight = Math.round(Math.min(width / first, (maxHeight - spacing) * 0.66))
  const halfWidth = (maxWidth - spacing) / 2
  const secondHeight = Math.min(
    maxHeight - firstHeight - spacing,
    Math.round(Math.min(halfWidth / second, halfWidth / third)),
  )
  const rightWidth = width - halfWidth - spacing
  return [
    { x: 0, y: 0, w: width, h: firstHeight, sides: SIDE_LEFT | SIDE_TOP | SIDE_RIGHT },
    { x: 0, y: firstHeight + spacing, w: halfWidth, h: secondHeight, sides: SIDE_LEFT | SIDE_BOTTOM },
    {
      x: halfWidth + spacing,
      y: firstHeight + spacing,
      w: rightWidth,
      h: secondHeight,
      sides: SIDE_RIGHT | SIDE_BOTTOM,
    },
  ]
}

function layoutFour(
  ratios: number[],
  proportions: string,
  { maxWidth, minWidth, spacing }: AlbumLayoutOptions,
  maxHeight: number,
): Rect[] {
  const [first, second, third, fourth] = ratios as [number, number, number, number]
  if (proportions[0] === 'w') {
    // One full-width item on top, three in a row below.
    const width = maxWidth
    const firstHeight = Math.round(Math.min(width / first, (maxHeight - spacing) * 0.66))
    const rowHeight = Math.round((maxWidth - 2 * spacing) / (second + third + fourth))
    const secondWidth = Math.max(minWidth, Math.round(Math.min((maxWidth - 2 * spacing) * 0.4, rowHeight * second)))
    const fourthWidth = Math.round(Math.max(Math.max(minWidth, (maxWidth - 2 * spacing) * 0.33), fourth * rowHeight))
    const thirdWidth = width - secondWidth - fourthWidth - 2 * spacing
    const height = Math.min(maxHeight - firstHeight - spacing, rowHeight)
    const y = firstHeight + spacing
    return [
      { x: 0, y: 0, w: width, h: firstHeight, sides: SIDE_LEFT | SIDE_TOP | SIDE_RIGHT },
      { x: 0, y, w: secondWidth, h: height, sides: SIDE_LEFT | SIDE_BOTTOM },
      { x: secondWidth + spacing, y, w: thirdWidth, h: height, sides: SIDE_BOTTOM },
      {
        x: secondWidth + thirdWidth + 2 * spacing,
        y,
        w: fourthWidth,
        h: height,
        sides: SIDE_RIGHT | SIDE_BOTTOM,
      },
    ]
  }
  // One full-height column on the left, three stacked on the right.
  const height = maxHeight
  const leftWidth = Math.round(Math.min(height * first, (maxWidth - spacing) * 0.6))
  const columnWidth = Math.round((maxHeight - 2 * spacing) / (1 / second + 1 / third + 1 / fourth))
  const secondHeight = Math.round(columnWidth / second)
  const thirdHeight = Math.round(columnWidth / third)
  const fourthHeight = height - secondHeight - thirdHeight - 2 * spacing
  const rightWidth = Math.max(minWidth, Math.min(maxWidth - leftWidth - spacing, columnWidth))
  const x = leftWidth + spacing
  return [
    { x: 0, y: 0, w: leftWidth, h: height, sides: SIDE_TOP | SIDE_LEFT | SIDE_BOTTOM },
    { x, y: 0, w: rightWidth, h: secondHeight, sides: SIDE_RIGHT | SIDE_TOP },
    { x, y: secondHeight + spacing, w: rightWidth, h: thirdHeight, sides: SIDE_RIGHT },
    {
      x,
      y: secondHeight + thirdHeight + 2 * spacing,
      w: rightWidth,
      h: fourthHeight,
      sides: SIDE_RIGHT | SIDE_BOTTOM,
    },
  ]
}

interface Attempt {
  lineCounts: number[]
  heights: number[]
}

/** Rows of 5–10 items (or any album with a very wide item): try every split, keep the best. */
function layoutComplex(
  ratios: number[],
  averageRatio: number,
  { maxWidth, minWidth, spacing }: AlbumLayoutOptions,
): Rect[] {
  const count = ratios.length
  // Very wide / very tall items are clamped so one cannot dominate the mosaic.
  const cropped = ratios.map((ratio) => (averageRatio > 1.1 ? Math.max(1, ratio) : Math.min(1, ratio)))
  const multiHeight = (from: number, to: number) => {
    let sum = 0
    for (let index = from; index < to; index += 1) sum += cropped[index]!
    return (maxWidth - (to - from - 1) * spacing) / sum
  }

  const attempts: Attempt[] = []
  const push = (lineCounts: number[]) => {
    const heights: number[] = []
    let offset = 0
    for (const lineCount of lineCounts) {
      heights.push(multiHeight(offset, offset + lineCount))
      offset += lineCount
    }
    attempts.push({ lineCounts, heights })
  }
  for (let first = 1; first < count; first += 1) {
    const second = count - first
    if (first > 3 || second > 3) continue
    push([first, second])
  }
  for (let first = 1; first < count - 1; first += 1) {
    for (let second = 1; second < count - first; second += 1) {
      const third = count - first - second
      if (first > 3 || second > (averageRatio < 0.85 ? 4 : 3) || third > 3) continue
      push([first, second, third])
    }
  }
  for (let first = 1; first < count - 1; first += 1) {
    for (let second = 1; second < count - first; second += 1) {
      for (let third = 1; third < count - first - second; third += 1) {
        const fourth = count - first - second - third
        if (first > 3 || second > 3 || third > 3 || fourth > 3) continue
        push([first, second, third, fourth])
      }
    }
  }

  // Score: distance of the total height from the target, heavily penalising rows shorter than
  // the minimum and reversed ordering (a longer row above a shorter one reads badly).
  const maxHeightTotal = (maxWidth / 3) * 4
  let best: Attempt | null = null
  let bestDiff = 0
  for (const attempt of attempts) {
    const totalHeight =
      attempt.heights.reduce((sum, height) => sum + height, 0) + spacing * (attempt.heights.length - 1)
    const minLineHeight = Math.min(...attempt.heights)
    const bad1 = minLineHeight < minWidth ? 1.5 : 1
    let bad2 = 1
    for (let line = 1; line < attempt.lineCounts.length; line += 1) {
      if (attempt.lineCounts[line - 1]! > attempt.lineCounts[line]!) {
        bad2 = 1.5
        break
      }
    }
    const diff = Math.abs(totalHeight - maxHeightTotal) * bad1 * bad2
    if (!best || diff < bestDiff) {
      best = attempt
      bestDiff = diff
    }
  }

  const rects: Rect[] = []
  let index = 0
  let y = 0
  const chosen = best!
  for (let row = 0; row < chosen.lineCounts.length; row += 1) {
    const lineCount = chosen.lineCounts[row]!
    const lineHeight = Math.round(chosen.heights[row]!)
    let x = 0
    for (let column = 0; column < lineCount; column += 1) {
      let sides = 0
      if (row === 0) sides |= SIDE_TOP
      if (row === chosen.lineCounts.length - 1) sides |= SIDE_BOTTOM
      if (column === 0) sides |= SIDE_LEFT
      // The last item of a row absorbs rounding so every row is exactly maxWidth wide.
      const isLast = column === lineCount - 1
      if (isLast) sides |= SIDE_RIGHT
      const width = isLast ? maxWidth - x : Math.round(cropped[index]! * lineHeight)
      rects.push({ x, y, w: width, h: lineHeight, sides })
      x += width + spacing
      index += 1
    }
    y += lineHeight + spacing
  }
  return rects
}

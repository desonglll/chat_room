/**
 * TG-403: the row-packing half of `layoutAlbum` — 5–10 items, or any album with a very wide
 * item. Every split into 2–4 rows of at most three (four in a narrow second row) is tried,
 * and the one whose total height is closest to the target wins, penalising rows shorter than
 * the minimum and a longer row above a shorter one. Split from `albumLayout.ts` for the
 * file-size gate; a port of Telegram Desktop's `LayoutMediaGroup` like its parent.
 */
import type { AlbumLayoutOptions, Rect } from './albumLayout'
import { SIDE_BOTTOM, SIDE_LEFT, SIDE_RIGHT, SIDE_TOP } from './albumLayout'

interface Attempt {
  lineCounts: number[]
  heights: number[]
}

/** Rows of 5–10 items (or any album with a very wide item): try every split, keep the best. */
export function layoutComplex(
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

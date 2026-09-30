/**
 * Masonry layout for the GIF panel: fixed-width columns, each GIF placed in the currently
 * shortest column at its own aspect ratio. Pure arithmetic, so the grid knows every cell's
 * box before any file loads, and the panel mounts only the cells near the viewport.
 */

export interface MasonryItem {
  /** Width / height; unknown geometry uses 1 until the media reports its own. */
  aspect: number | null
}

export interface MasonryBox {
  x: number
  y: number
  width: number
  height: number
}

export interface MasonryLayout {
  columns: number
  boxes: MasonryBox[]
  height: number
}

export interface MasonryOptions {
  width: number
  gap: number
  /** Columns are as many as fit at this width or wider, and at least two. */
  minColumnWidth: number
}

/** Very wide or very tall GIFs are clamped so no cell is a sliver or a tower. */
export const MIN_ASPECT = 0.5
export const MAX_ASPECT = 2.5

export function clampAspect(aspect: number | null): number {
  if (aspect === null || !Number.isFinite(aspect) || aspect <= 0) return 1
  return Math.min(MAX_ASPECT, Math.max(MIN_ASPECT, aspect))
}

export function masonryLayout(items: readonly MasonryItem[], options: MasonryOptions): MasonryLayout {
  const { gap } = options
  const width = Math.max(0, options.width)
  const columns = Math.max(2, Math.floor((width + gap) / (options.minColumnWidth + gap)))
  const columnWidth = Math.max(0, (width - gap * (columns - 1)) / columns)
  const heights = new Array<number>(columns).fill(0)
  const boxes = items.map((item) => {
    let column = 0
    for (let index = 1; index < columns; index += 1) {
      if ((heights[index] ?? 0) < (heights[column] ?? 0)) column = index
    }
    const y = heights[column] ?? 0
    const height = Math.round(columnWidth / clampAspect(item.aspect))
    heights[column] = y + height + gap
    return { x: Math.round(column * (columnWidth + gap)), y, width: Math.round(columnWidth), height }
  })
  const height = Math.max(0, ...heights.map((value) => value - gap))
  return { columns, boxes, height: items.length === 0 ? 0 : height }
}

/** Indices of the boxes that intersect `[top - overscan, top + viewport + overscan]`. */
export function visibleBoxes(boxes: readonly MasonryBox[], top: number, viewport: number, overscan: number): number[] {
  const from = top - overscan
  const to = top + viewport + overscan
  const visible: number[] = []
  boxes.forEach((box, index) => {
    if (box.y + box.height >= from && box.y <= to) visible.push(index)
  })
  return visible
}

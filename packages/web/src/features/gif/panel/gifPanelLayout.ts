/**
 * The whole GIF tab as one scrollable column: each non-empty section is a header followed
 * by its own masonry. Pure, so the grid knows every cell's absolute box up front.
 */
import { masonryLayout, type MasonryBox } from './masonry'

export interface GifCell {
  key: string
  aspect: number | null
}

export interface GifSection<T extends GifCell = GifCell> {
  id: string
  title: string
  items: readonly T[]
}

export interface PlacedGif<T extends GifCell = GifCell> {
  item: T
  sectionId: string
  box: MasonryBox
}

export interface PlacedHeader {
  id: string
  title: string
  top: number
}

export interface GifPanelLayout<T extends GifCell = GifCell> {
  headers: PlacedHeader[]
  cells: PlacedGif<T>[]
  height: number
}

export const HEADER_HEIGHT = 32
export const GIF_GAP = 4
export const MIN_COLUMN_WIDTH = 100

export function gifPanelLayout<T extends GifCell>(
  sections: readonly GifSection<T>[],
  width: number,
): GifPanelLayout<T> {
  const headers: PlacedHeader[] = []
  const cells: PlacedGif<T>[] = []
  let top = 0
  for (const section of sections) {
    if (section.items.length === 0) continue
    headers.push({ id: section.id, title: section.title, top })
    top += HEADER_HEIGHT
    const layout = masonryLayout(section.items, { width, gap: GIF_GAP, minColumnWidth: MIN_COLUMN_WIDTH })
    section.items.forEach((item, index) => {
      const box = layout.boxes[index]
      if (box) cells.push({ item, sectionId: section.id, box: { ...box, y: box.y + top } })
    })
    top += layout.height + GIF_GAP
  }
  return { headers, cells, height: top }
}

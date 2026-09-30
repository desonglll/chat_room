/**
 * Geometry of the sticker tab's virtual list, as pure functions.
 *
 * The tab is one scroll container holding sections (favorites, recents, each installed set)
 * of fixed-size cells. Everything is fixed height, so the whole layout is arithmetic: a flat
 * list of header and row items with absolute offsets. Only the items that intersect the
 * viewport (plus overscan) are mounted, which is what keeps opening a panel with hundreds of
 * stickers cheap — TG-301's manager fetches and parses a sticker as soon as its view
 * attaches, so an unmounted row costs nothing at all.
 */
import type { Sticker } from '@tg/core'

export interface PanelSection {
  /** `favorites`, `recent`, or a set id. */
  id: string
  title: string
  stickers: readonly Sticker[]
}

export type PanelItem =
  | { kind: 'header'; key: string; sectionId: string; title: string; top: number; height: number }
  | { kind: 'row'; key: string; sectionId: string; stickers: readonly Sticker[]; top: number; height: number }

export interface PanelLayout {
  items: PanelItem[]
  /** Section id → offset of its header, in section order. */
  sectionTops: Array<{ id: string; top: number }>
  height: number
  columns: number
}

export interface PanelMetrics {
  headerHeight: number
  cellSize: number
  /** Padding below the last row of a section. */
  sectionGap: number
}

export const PANEL_METRICS: PanelMetrics = { headerHeight: 32, cellSize: 72, sectionGap: 8 }

export function columnsFor(width: number, cellSize = PANEL_METRICS.cellSize): number {
  return Math.max(1, Math.floor(width / cellSize))
}

export function layoutPanel(
  sections: readonly PanelSection[],
  columns: number,
  metrics: PanelMetrics = PANEL_METRICS,
): PanelLayout {
  const items: PanelItem[] = []
  const sectionTops: PanelLayout['sectionTops'] = []
  let top = 0
  for (const section of sections) {
    if (section.stickers.length === 0) continue
    sectionTops.push({ id: section.id, top })
    // An untitled section (a single set shown in its own dialog) gets no header row.
    if (section.title !== '') {
      items.push({
        kind: 'header',
        key: `h:${section.id}`,
        sectionId: section.id,
        title: section.title,
        top,
        height: metrics.headerHeight,
      })
      top += metrics.headerHeight
    }
    for (let start = 0; start < section.stickers.length; start += columns) {
      items.push({
        kind: 'row',
        key: `r:${section.id}:${start}`,
        sectionId: section.id,
        stickers: section.stickers.slice(start, start + columns),
        top,
        height: metrics.cellSize,
      })
      top += metrics.cellSize
    }
    top += metrics.sectionGap
  }
  return { items, sectionTops, height: top, columns }
}

/** Items intersecting `[scrollTop − overscan, scrollTop + viewport + overscan]`, by binary search. */
export function visibleItems(layout: PanelLayout, scrollTop: number, viewport: number, overscan: number): PanelItem[] {
  const from = scrollTop - overscan
  const to = scrollTop + viewport + overscan
  const { items } = layout
  let low = 0
  let high = items.length
  while (low < high) {
    const middle = (low + high) >> 1
    const item = items[middle]!
    if (item.top + item.height <= from) low = middle + 1
    else high = middle
  }
  const visible: PanelItem[] = []
  for (let index = low; index < items.length; index += 1) {
    const item = items[index]!
    if (item.top >= to) break
    visible.push(item)
  }
  return visible
}

/** The section the rail highlights: the last one whose header is at or above the top edge. */
export function sectionAt(layout: PanelLayout, scrollTop: number): string | null {
  let active: string | null = layout.sectionTops[0]?.id ?? null
  for (const { id, top } of layout.sectionTops) {
    if (top <= scrollTop + 1) active = id
    else break
  }
  return active
}

export function sectionTop(layout: PanelLayout, sectionId: string): number | null {
  return layout.sectionTops.find((entry) => entry.id === sectionId)?.top ?? null
}

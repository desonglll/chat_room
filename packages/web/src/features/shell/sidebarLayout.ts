/**
 * Sidebar width and collapse, as Telegram Web does it: drag the divider between a
 * minimum and a maximum; dragging well below the minimum snaps to the collapsed
 * avatar-only column. Persisted through the injected `CoreStorage` — never a global.
 */
import type { CoreStorage } from '@tg/core'

export const SIDEBAR_STORAGE_KEY = 'tg.sidebar.v1'
export const SIDEBAR_MIN_WIDTH = 260
export const SIDEBAR_MAX_WIDTH = 480
export const SIDEBAR_DEFAULT_WIDTH = 360
/** Below this while dragging, the column collapses (and above it, expands again). */
export const SIDEBAR_COLLAPSE_THRESHOLD = 180
/** The collapsed column: one 54px avatar plus row padding. */
export const SIDEBAR_COLLAPSED_WIDTH = 80

export interface SidebarLayout {
  width: number
  collapsed: boolean
}

export const DEFAULT_SIDEBAR_LAYOUT: SidebarLayout = { width: SIDEBAR_DEFAULT_WIDTH, collapsed: false }

export const clampSidebarWidth = (width: number): number =>
  Math.round(Math.min(SIDEBAR_MAX_WIDTH, Math.max(SIDEBAR_MIN_WIDTH, width)))

/**
 * Where a drag at `pointerWidth` (the pointer's distance from the sidebar's left edge)
 * leaves the layout. Collapsing keeps the last expanded width so un-collapsing restores it.
 */
export function layoutForDrag(current: SidebarLayout, pointerWidth: number): SidebarLayout {
  if (pointerWidth < SIDEBAR_COLLAPSE_THRESHOLD) return { width: current.width, collapsed: true }
  return { width: clampSidebarWidth(pointerWidth), collapsed: false }
}

export function readSidebarLayout(storage: CoreStorage): SidebarLayout {
  const raw = storage.getItem(SIDEBAR_STORAGE_KEY)
  if (!raw) return DEFAULT_SIDEBAR_LAYOUT
  try {
    const parsed = JSON.parse(raw) as Partial<SidebarLayout> | null
    if (typeof parsed !== 'object' || parsed === null) return DEFAULT_SIDEBAR_LAYOUT
    const width =
      typeof parsed.width === 'number' && Number.isFinite(parsed.width) ? parsed.width : SIDEBAR_DEFAULT_WIDTH
    return { width: clampSidebarWidth(width), collapsed: parsed.collapsed === true }
  } catch {
    return DEFAULT_SIDEBAR_LAYOUT
  }
}

export function writeSidebarLayout(storage: CoreStorage, layout: SidebarLayout): void {
  storage.setItem(
    SIDEBAR_STORAGE_KEY,
    JSON.stringify({ width: clampSidebarWidth(layout.width), collapsed: layout.collapsed }),
  )
}

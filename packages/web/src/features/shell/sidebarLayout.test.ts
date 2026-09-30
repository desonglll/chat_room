/** Sidebar width/collapse rules and their persistence through the injected storage. */
import { describe, expect, test } from 'bun:test'
import type { CoreStorage } from '@tg/core'
import {
  DEFAULT_SIDEBAR_LAYOUT,
  SIDEBAR_MAX_WIDTH,
  SIDEBAR_MIN_WIDTH,
  SIDEBAR_STORAGE_KEY,
  clampSidebarWidth,
  layoutForDrag,
  readSidebarLayout,
  writeSidebarLayout,
} from './sidebarLayout'

const memoryStorage = (): CoreStorage & { data: Map<string, string> } => {
  const data = new Map<string, string>()
  return {
    data,
    getItem: (key) => data.get(key) ?? null,
    setItem: (key, value) => void data.set(key, value),
    removeItem: (key) => void data.delete(key),
  }
}

describe('sidebar layout', () => {
  test('width clamps to the Telegram range and rounds', () => {
    expect(clampSidebarWidth(10)).toBe(SIDEBAR_MIN_WIDTH)
    expect(clampSidebarWidth(9_999)).toBe(SIDEBAR_MAX_WIDTH)
    expect(clampSidebarWidth(300.6)).toBe(301)
  })

  test('dragging past the threshold collapses and keeps the last width; dragging back expands', () => {
    const collapsed = layoutForDrag({ width: 400, collapsed: false }, 120)
    expect(collapsed).toEqual({ width: 400, collapsed: true })
    expect(layoutForDrag(collapsed, 200)).toEqual({ width: SIDEBAR_MIN_WIDTH, collapsed: false })
    expect(layoutForDrag(collapsed, 333)).toEqual({ width: 333, collapsed: false })
  })

  test('round-trips through storage; corrupt or missing data falls back to the default', () => {
    const storage = memoryStorage()
    expect(readSidebarLayout(storage)).toEqual(DEFAULT_SIDEBAR_LAYOUT)
    writeSidebarLayout(storage, { width: 412, collapsed: true })
    expect(readSidebarLayout(storage)).toEqual({ width: 412, collapsed: true })
    storage.setItem(SIDEBAR_STORAGE_KEY, '{nope')
    expect(readSidebarLayout(storage)).toEqual(DEFAULT_SIDEBAR_LAYOUT)
    storage.setItem(SIDEBAR_STORAGE_KEY, JSON.stringify({ width: 'wide', collapsed: 'yes' }))
    expect(readSidebarLayout(storage)).toEqual(DEFAULT_SIDEBAR_LAYOUT)
    storage.setItem(SIDEBAR_STORAGE_KEY, JSON.stringify({ width: 5000 }))
    expect(readSidebarLayout(storage)).toEqual({ width: SIDEBAR_MAX_WIDTH, collapsed: false })
  })
})

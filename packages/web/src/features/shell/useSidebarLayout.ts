/**
 * The sidebar's width/collapse state: hydrated from the injected storage, mirrored into
 * `uiStore` (the cross-feature read point), persisted when a drag or toggle settles —
 * not on every pointer move.
 */
import { useCallback, useEffect, useState } from 'react'
import { uiStore } from '@tg/core'
import { browserStorage } from '../../app/platform'
import type { SidebarLayout } from './sidebarLayout'
import { readSidebarLayout, writeSidebarLayout } from './sidebarLayout'

export interface SidebarLayoutControls {
  layout: SidebarLayout
  /** Live update (during a drag). */
  preview: (layout: SidebarLayout) => void
  /** Settled update: applies and persists. */
  commit: (layout: SidebarLayout) => void
  toggleCollapsed: () => void
}

export function useSidebarLayout(): SidebarLayoutControls {
  const [layout, setLayout] = useState<SidebarLayout>(() => readSidebarLayout(browserStorage))

  useEffect(() => {
    uiStore.getState().setSidebarWidth(layout.width)
    uiStore.getState().setSidebarCollapsed(layout.collapsed)
  }, [layout])

  const commit = useCallback((next: SidebarLayout) => {
    setLayout(next)
    writeSidebarLayout(browserStorage, next)
  }, [])

  const toggleCollapsed = useCallback(() => {
    setLayout((current) => {
      const next = { ...current, collapsed: !current.collapsed }
      writeSidebarLayout(browserStorage, next)
      return next
    })
  }, [])

  return { layout, preview: setLayout, commit, toggleCollapsed }
}

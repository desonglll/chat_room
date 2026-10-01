/**
 * The draggable divider on the sidebar's right edge. Pointer drag resizes (and past the
 * collapse threshold, collapses); keyboard users get the same through the ARIA
 * `separator` pattern: ←/→ resize, Enter toggles collapse, Home resets.
 */
import type { KeyboardEvent, PointerEvent } from 'react'
import { useRef } from 'react'
import type { SidebarLayout } from './sidebarLayout'
import {
  DEFAULT_SIDEBAR_LAYOUT,
  SIDEBAR_MAX_WIDTH,
  SIDEBAR_MIN_WIDTH,
  clampSidebarWidth,
  layoutForDrag,
} from './sidebarLayout'
import { t } from '../../i18n/index'

export interface SidebarResizerProps {
  layout: SidebarLayout
  onPreview: (layout: SidebarLayout) => void
  onCommit: (layout: SidebarLayout) => void
}

const KEY_STEP = 16

export function SidebarResizer({ layout, onPreview, onCommit }: SidebarResizerProps) {
  const drag = useRef<{ left: number; latest: SidebarLayout } | null>(null)

  const onPointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) return
    const sidebar = event.currentTarget.parentElement
    drag.current = { left: sidebar?.getBoundingClientRect().left ?? 0, latest: layout }
    event.currentTarget.setPointerCapture(event.pointerId)
    event.preventDefault()
  }

  const onPointerMove = (event: PointerEvent<HTMLDivElement>) => {
    const current = drag.current
    if (!current) return
    const next = layoutForDrag(current.latest, event.clientX - current.left)
    if (next.width === current.latest.width && next.collapsed === current.latest.collapsed) return
    current.latest = next
    onPreview(next)
  }

  const endDrag = (event: PointerEvent<HTMLDivElement>) => {
    const current = drag.current
    if (!current) return
    drag.current = null
    if (event.currentTarget.hasPointerCapture(event.pointerId))
      event.currentTarget.releasePointerCapture(event.pointerId)
    onCommit(current.latest)
  }

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    let next: SidebarLayout | null = null
    if (event.key === 'ArrowLeft') next = { collapsed: false, width: clampSidebarWidth(layout.width - KEY_STEP) }
    else if (event.key === 'ArrowRight') next = { collapsed: false, width: clampSidebarWidth(layout.width + KEY_STEP) }
    else if (event.key === 'Enter') next = { ...layout, collapsed: !layout.collapsed }
    else if (event.key === 'Home') next = DEFAULT_SIDEBAR_LAYOUT
    if (!next) return
    event.preventDefault()
    onCommit(next)
  }

  return (
    <div
      className="tg-shell__resizer"
      role="separator"
      aria-orientation="vertical"
      aria-label={t('w.shell.169df2')}
      aria-valuemin={SIDEBAR_MIN_WIDTH}
      aria-valuemax={SIDEBAR_MAX_WIDTH}
      aria-valuenow={layout.collapsed ? SIDEBAR_MIN_WIDTH : layout.width}
      tabIndex={0}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={endDrag}
      onPointerCancel={endDrag}
      onDoubleClick={() => onCommit(DEFAULT_SIDEBAR_LAYOUT)}
      onKeyDown={onKeyDown}
    />
  )
}

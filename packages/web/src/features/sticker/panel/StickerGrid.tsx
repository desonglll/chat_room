/**
 * The sticker tab's scroll area: a fixed-geometry virtual list (see `panelLayout.ts`) of
 * section headers and rows of sticker buttons. Only the rows near the viewport are mounted.
 *
 * - `onActiveSection` reports the section at the top edge, for the rail.
 * - `scrollToSection` (through `handleRef`) is how the rail jumps; headers are scroll-snap
 *   points (`proximity`), so a fling settles on a section boundary as in Telegram.
 * - Press-and-hold previews (`usePressPreview`), right click opens `onMenu`.
 */
import { useEffect, useMemo, useRef, useState, type RefObject } from 'react'
import type { Sticker } from '@tg/core'
import { StickerPreview } from '../preview/StickerPreview'
import { usePressPreview } from '../preview/usePressPreview'
import { StickerView } from '../StickerView'
import {
  columnsFor,
  layoutPanel,
  PANEL_METRICS,
  sectionAt,
  sectionTop,
  visibleItems,
  type PanelSection,
} from './panelLayout'
import { t } from '../../../i18n/index'

export interface StickerGridHandle {
  scrollToSection(sectionId: string): void
}

export interface StickerGridProps {
  sections: readonly PanelSection[]
  onPick(sticker: Sticker): void
  onMenu?: ((sticker: Sticker, point: { x: number; y: number }) => void) | undefined
  onActiveSection?: ((sectionId: string | null) => void) | undefined
  handleRef?: RefObject<StickerGridHandle | null> | undefined
  disabled?: boolean | undefined
  /** Initial viewport guess before the first measurement (and in static rendering). */
  initialViewport?: { width: number; height: number } | undefined
  label: string
}

const CELL_STICKER = 64
/** The media panel's grid box (panel.css) — right on the first render, corrected by the observer. */
const DEFAULT_VIEWPORT = { width: 312, height: 300 }
const OVERSCAN = 144

const reducedMotion = () =>
  typeof window !== 'undefined' && window.matchMedia?.('(prefers-reduced-motion: reduce)').matches === true

export function StickerGrid(props: StickerGridProps) {
  const { sections, onPick, onMenu, onActiveSection, handleRef, disabled = false, label } = props
  const scrollRef = useRef<HTMLDivElement>(null)
  const [viewport, setViewport] = useState(props.initialViewport ?? DEFAULT_VIEWPORT)
  const [scrollTop, setScrollTop] = useState(0)
  const columns = columnsFor(viewport.width)
  const layout = useMemo(() => layoutPanel(sections, columns), [sections, columns])
  const byId = useMemo(() => {
    const map = new Map<string, Sticker>()
    for (const section of sections) for (const sticker of section.stickers) map.set(sticker.id, sticker)
    return map
  }, [sections])
  const preview = usePressPreview()

  // Measured by ResizeObserver only: its first callback arrives after the browser's own
  // layout, so opening the panel never forces a synchronous layout from script.
  useEffect(() => {
    const element = scrollRef.current
    if (!element) return
    const observer = new ResizeObserver(([entry]) => {
      const box = entry?.contentBoxSize?.[0]
      const width = Math.round(box?.inlineSize ?? element.clientWidth)
      const height = Math.round(box?.blockSize ?? element.clientHeight)
      setViewport((current) => (current.width === width && current.height === height ? current : { width, height }))
    })
    observer.observe(element)
    return () => observer.disconnect()
  }, [])

  useEffect(() => {
    const element = scrollRef.current
    if (!element) return
    let frame = 0
    const onScroll = () => {
      if (frame !== 0) return
      frame = requestAnimationFrame(() => {
        frame = 0
        setScrollTop(element.scrollTop)
      })
    }
    element.addEventListener('scroll', onScroll, { passive: true })
    return () => {
      element.removeEventListener('scroll', onScroll)
      cancelAnimationFrame(frame)
    }
  }, [])

  const active = sectionAt(layout, scrollTop)
  const activeRef = useRef(onActiveSection)
  activeRef.current = onActiveSection
  useEffect(() => activeRef.current?.(active), [active])

  useEffect(() => {
    if (!handleRef) return
    handleRef.current = {
      scrollToSection(sectionId) {
        const top = sectionTop(layout, sectionId)
        if (top === null) return
        scrollRef.current?.scrollTo({ top, behavior: reducedMotion() ? 'auto' : 'smooth' })
      },
    }
  }, [handleRef, layout])

  const items = visibleItems(layout, scrollTop, viewport.height, OVERSCAN)
  const cellWidth = viewport.width / columns

  return (
    <div
      ref={scrollRef}
      className="tg-sticker-grid"
      role="grid"
      aria-label={label}
      {...preview.handlers}
      onContextMenu={(event) => {
        const id = ((event.target as HTMLElement).closest('[data-sticker-id]') as HTMLElement | null)?.dataset.stickerId
        const sticker = id ? byId.get(id) : undefined
        if (!sticker) return
        event.preventDefault()
        if (preview.previewId === null) onMenu?.(sticker, { x: event.clientX, y: event.clientY })
      }}
    >
      <div className="tg-sticker-grid__canvas" style={{ height: layout.height }}>
        {items.map((item) =>
          item.kind === 'header' ? (
            <div
              key={item.key}
              className="tg-sticker-grid__header"
              data-section={item.sectionId}
              style={{ top: item.top, height: item.height }}
            >
              {item.title}
            </div>
          ) : (
            <div
              key={item.key}
              role="row"
              className="tg-sticker-grid__row"
              style={{ top: item.top, height: item.height }}
            >
              {item.stickers.map((sticker) => (
                <button
                  key={sticker.id}
                  type="button"
                  role="gridcell"
                  className="tg-sticker-grid__cell"
                  style={{ width: cellWidth }}
                  data-sticker-id={sticker.id}
                  aria-label={t('w.sticker.3c8532', sticker.emoji)}
                  disabled={disabled}
                  onClick={() => onPick(sticker)}
                >
                  <StickerView
                    src={sticker.file_url}
                    format={sticker.format}
                    size={CELL_STICKER}
                    label={sticker.emoji}
                  />
                </button>
              ))}
            </div>
          ),
        )}
      </div>
      <StickerPreview sticker={preview.previewId ? (byId.get(preview.previewId) ?? null) : null} />
    </div>
  )
}

export { PANEL_METRICS }

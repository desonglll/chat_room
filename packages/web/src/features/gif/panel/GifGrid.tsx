/**
 * The virtual masonry grid of the GIF tab. Every cell's box comes from `gifPanelLayout`,
 * and only the cells within the viewport ± one screen are mounted, so a long saved list
 * costs a handful of `<video>` elements, not hundreds.
 */
import { useEffect, useMemo, useRef, useState, type MouseEvent } from 'react'
import { GifPlayer } from '../GifPlayer'
import { gifPanelLayout, type GifCell, type GifSection } from './gifPanelLayout'
import { visibleBoxes } from './masonry'
import { t } from '../../../i18n/index'

export interface GifGridItem extends GifCell {
  src: string
  mimeType: string
}

export interface GifGridProps<T extends GifGridItem> {
  sections: readonly GifSection<T>[]
  disabled: boolean
  onPick(item: T): void
  onMenu(item: T, point: { x: number; y: number }): void
  onAspect(item: T, aspect: number): void
}

/** Matches `.tg-gif-tab__grid`'s width before the first ResizeObserver callback. */
const INITIAL_WIDTH = 340
const INITIAL_HEIGHT = 360

export function GifGrid<T extends GifGridItem>({ sections, disabled, onPick, onMenu, onAspect }: GifGridProps<T>) {
  const scrollRef = useRef<HTMLDivElement | null>(null)
  const [viewport, setViewport] = useState({ width: INITIAL_WIDTH, height: INITIAL_HEIGHT })
  const [scrollTop, setScrollTop] = useState(0)
  const layout = useMemo(() => gifPanelLayout(sections, viewport.width), [sections, viewport.width])
  const mounted = useMemo(
    () =>
      new Set(
        visibleBoxes(
          layout.cells.map((cell) => cell.box),
          scrollTop,
          viewport.height,
          viewport.height,
        ),
      ),
    [layout, scrollTop, viewport.height],
  )

  useEffect(() => {
    const element = scrollRef.current
    if (!element || typeof ResizeObserver === 'undefined') return
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
      if (frame) return
      frame = requestAnimationFrame(() => {
        frame = 0
        setScrollTop(element.scrollTop)
      })
    }
    element.addEventListener('scroll', onScroll, { passive: true })
    return () => {
      element.removeEventListener('scroll', onScroll)
      if (frame) cancelAnimationFrame(frame)
    }
  }, [])

  const menu = (item: T) => (event: MouseEvent) => {
    event.preventDefault()
    onMenu(item, { x: event.clientX, y: event.clientY })
  }

  return (
    <div ref={scrollRef} className="tg-gif-tab__grid" role="list" aria-label="GIF">
      <div className="tg-gif-tab__canvas" style={{ blockSize: layout.height }}>
        {layout.headers.map((header) => (
          <h3 key={header.id} className="tg-gif-tab__header" style={{ insetBlockStart: header.top }}>
            {header.title}
          </h3>
        ))}
        {layout.cells.map((cell, index) =>
          mounted.has(index) ? (
            <button
              key={`${cell.sectionId}:${cell.item.key}`}
              type="button"
              role="listitem"
              className="tg-gif-tab__cell"
              disabled={disabled}
              style={{
                insetInlineStart: cell.box.x,
                insetBlockStart: cell.box.y,
                inlineSize: cell.box.width,
                blockSize: cell.box.height,
              }}
              aria-label={t('w.gif.f02ec6')}
              onClick={() => onPick(cell.item)}
              onContextMenu={menu(cell.item)}
            >
              <GifPlayer
                src={cell.item.src}
                mimeType={cell.item.mimeType}
                label="GIF"
                className="tg-gif-tab__player"
                onAspect={(aspect) => onAspect(cell.item, aspect)}
              />
            </button>
          ) : null,
        )}
      </div>
    </div>
  )
}

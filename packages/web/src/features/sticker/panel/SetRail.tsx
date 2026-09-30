/**
 * The vertical rail beside the sticker grid: favorites, recents, then one button per
 * installed set (its first sticker, drawn still), and the settings gear at the bottom.
 * The active entry follows the grid's scroll position and keeps itself in view.
 */
import { useEffect, useRef } from 'react'
import type { Sticker } from '@tg/core'
import { ClockGlyph, GearGlyph, StarGlyph } from '../icons'
import { StickerView } from '../StickerView'

export interface RailEntry {
  id: string
  title: string
  /** A glyph for the built-in sections, a sticker thumbnail for a set. */
  icon: 'favorites' | 'recent' | Sticker
}

export interface SetRailProps {
  entries: readonly RailEntry[]
  active: string | null
  onSelect(sectionId: string): void
  onSettings?: (() => void) | undefined
}

const THUMB = 32

export function SetRail({ entries, active, onSelect, onSettings }: SetRailProps) {
  const listRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!active) return
    const button = listRef.current?.querySelector<HTMLElement>(`[data-rail-id="${CSS.escape(active)}"]`)
    button?.scrollIntoView({ block: 'nearest', inline: 'nearest' })
  }, [active])

  return (
    <nav className="tg-sticker-rail" aria-label="贴纸包">
      <div className="tg-sticker-rail__list" ref={listRef}>
        {entries.map((entry) => (
          <button
            key={entry.id}
            type="button"
            className="tg-sticker-rail__item"
            data-rail-id={entry.id}
            aria-current={entry.id === active ? 'true' : undefined}
            aria-label={entry.title}
            title={entry.title}
            onClick={() => onSelect(entry.id)}
          >
            {entry.icon === 'favorites' ? (
              <StarGlyph />
            ) : entry.icon === 'recent' ? (
              <ClockGlyph />
            ) : (
              <StickerView
                src={entry.icon.file_url}
                format={entry.icon.format}
                size={THUMB}
                label={entry.icon.emoji}
                autoplay={false}
                loop={false}
              />
            )}
          </button>
        ))}
      </div>
      {onSettings ? (
        <button
          type="button"
          className="tg-sticker-rail__item"
          aria-label="贴纸设置"
          title="贴纸设置"
          onClick={onSettings}
        >
          <GearGlyph />
        </button>
      ) : null}
    </nav>
  )
}

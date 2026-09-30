/**
 * The bottom strip of the Chat's media. Windowed (a Chat can hold thousands of photos) and
 * kept centred on the current item.
 */
import { useEffect, useRef } from 'react'
import type { MediaItem } from './mediaItem'
import { stripWindow } from './mediaNavigation'
import { PlayGlyph } from './icons'

/** Items rendered either side of the current one. */
export const STRIP_RADIUS = 20

export interface ThumbStripProps {
  items: readonly MediaItem[]
  index: number
  onSelect(attachmentId: string): void
  reduced: boolean
}

export function ThumbStrip({ items, index, onSelect, reduced }: ThumbStripProps) {
  const currentRef = useRef<HTMLButtonElement>(null)
  const currentId = items[index]?.attachmentId

  useEffect(() => {
    currentRef.current?.scrollIntoView({ inline: 'center', block: 'nearest', behavior: reduced ? 'auto' : 'smooth' })
  }, [currentId, reduced])

  if (items.length < 2 || index < 0) return null
  const { start, end } = stripWindow(items.length, index, STRIP_RADIUS)
  return (
    <nav className="tg-mv__strip" aria-label="本聊天的媒体" data-mv-chrome="">
      {items.slice(start, end).map((item) => {
        const current = item.attachmentId === currentId
        return (
          <button
            key={item.attachmentId}
            ref={current ? currentRef : undefined}
            type="button"
            className="tg-mv__thumb"
            aria-current={current ? 'true' : undefined}
            aria-label={`${item.kind === 'image' ? '图片' : '视频'} ${item.fileName}`}
            data-sensitive={item.isSensitive ? '' : undefined}
            onClick={() => onSelect(item.attachmentId)}
          >
            {item.kind === 'image' ? (
              <img src={item.url} alt="" loading="lazy" decoding="async" draggable={false} />
            ) : (
              <>
                <video src={item.url} preload="metadata" muted playsInline />
                <span className="tg-mv__thumb-play">
                  <PlayGlyph />
                </span>
              </>
            )}
          </button>
        )
      })}
    </nav>
  )
}

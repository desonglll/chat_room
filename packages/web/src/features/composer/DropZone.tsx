/**
 * Drag-and-drop upload. While files are dragged over the window, two Telegram-style
 * zones cover the chat column the composer lives in: «以文件形式发送» and «快速发送»
 * (photos/videos). Dropping outside the zones is swallowed so the browser never
 * navigates away to the file. Only `Files` drags react — dragging text is left alone.
 */
import { useEffect, useRef, useState, type DragEvent, type RefObject } from 'react'
import { UploadGlyph } from './icons'
import { t } from '../../i18n/index'

export interface DropZoneProps {
  /** The composer root; the overlay covers its parent (the chat column). */
  anchorRef: RefObject<HTMLElement | null>
  disabled?: boolean
  onFiles(files: File[], asFiles: boolean): void
}

interface Rect {
  top: number
  left: number
  width: number
  height: number
}

const hasFiles = (event: globalThis.DragEvent) => event.dataTransfer?.types.includes('Files') ?? false

export function DropZone({ anchorRef, disabled = false, onFiles }: DropZoneProps) {
  const [rect, setRect] = useState<Rect | null>(null)
  const depth = useRef(0)

  useEffect(() => {
    if (disabled) return
    const onEnter = (event: globalThis.DragEvent) => {
      if (!hasFiles(event)) return
      depth.current += 1
      const column = anchorRef.current?.parentElement ?? anchorRef.current
      if (!column) return
      const box = column.getBoundingClientRect()
      setRect({ top: box.top, left: box.left, width: box.width, height: box.height })
    }
    const onLeave = (event: globalThis.DragEvent) => {
      if (!hasFiles(event)) return
      depth.current = Math.max(0, depth.current - 1)
      if (depth.current === 0) setRect(null)
    }
    const onOver = (event: globalThis.DragEvent) => {
      if (hasFiles(event)) event.preventDefault()
    }
    const onDrop = (event: globalThis.DragEvent) => {
      if (!hasFiles(event)) return
      event.preventDefault()
      depth.current = 0
      setRect(null)
    }
    window.addEventListener('dragenter', onEnter)
    window.addEventListener('dragleave', onLeave)
    window.addEventListener('dragover', onOver)
    window.addEventListener('drop', onDrop)
    return () => {
      window.removeEventListener('dragenter', onEnter)
      window.removeEventListener('dragleave', onLeave)
      window.removeEventListener('dragover', onOver)
      window.removeEventListener('drop', onDrop)
    }
  }, [anchorRef, disabled])

  if (!rect) return null

  const zone = (asFiles: boolean, title: string, hint: string) => (
    <div
      className="tg-compose__drop-zone"
      onDragOver={(event: DragEvent) => {
        event.preventDefault()
        event.currentTarget.setAttribute('data-over', '')
      }}
      onDragLeave={(event: DragEvent) => event.currentTarget.removeAttribute('data-over')}
      onDrop={(event: DragEvent) => {
        event.preventDefault()
        const files = Array.from(event.dataTransfer.files)
        if (files.length > 0) onFiles(files, asFiles)
      }}
    >
      <UploadGlyph />
      <span className="tg-compose__drop-title">{title}</span>
      <span className="tg-compose__drop-hint">{hint}</span>
    </div>
  )

  return (
    <div className="tg-compose__drop" style={rect} aria-hidden="true">
      {zone(true, t('w.composer.a3c36d'), t('w.composer.e02394'))}
      {zone(false, t('w.composer.a3c36d'), t('w.composer.8bfdcf'))}
    </div>
  )
}

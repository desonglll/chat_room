/**
 * Binds `createPressPreview` to a container by event delegation: any descendant carrying
 * `data-sticker-id` is previewable, so a grid of hundreds of cells adds no per-cell
 * listeners. Returns the handlers to spread on the container and the id being previewed.
 */
import { useEffect, useMemo, useRef, useState, type MouseEvent, type PointerEvent } from 'react'
import { browserClock } from '../../../app/platform'
import { createPressPreview } from './pressPreview'

const idAt = (x: number, y: number): string | null =>
  (document.elementFromPoint(x, y)?.closest('[data-sticker-id]') as HTMLElement | null)?.dataset.stickerId ?? null

export function usePressPreview() {
  const [previewId, setPreviewId] = useState<string | null>(null)
  const machine = useMemo(() => createPressPreview({ clock: browserClock, onPreview: setPreviewId }), [])
  const swallowClick = useRef(false)
  useEffect(() => () => machine.cancel(), [machine])

  const handlers = {
    onPointerDown(event: PointerEvent<HTMLElement>) {
      if (event.button !== 0) return
      const cell = (event.target as HTMLElement).closest('[data-sticker-id]') as HTMLElement | null
      const id = cell?.dataset.stickerId
      if (id) machine.down(id, event.clientX, event.clientY)
    },
    onPointerMove(event: PointerEvent<HTMLElement>) {
      machine.move(event.clientX, event.clientY, machine.open === null ? null : idAt(event.clientX, event.clientY))
    },
    onPointerUp() {
      if (machine.up()) swallowClick.current = true
    },
    onPointerCancel() {
      machine.cancel()
    },
    onPointerLeave() {
      if (machine.open === null) machine.cancel()
    },
    onClickCapture(event: MouseEvent<HTMLElement>) {
      if (!swallowClick.current) return
      swallowClick.current = false
      event.preventDefault()
      event.stopPropagation()
    },
  }
  return { previewId, handlers, closePreview: () => machine.cancel() }
}

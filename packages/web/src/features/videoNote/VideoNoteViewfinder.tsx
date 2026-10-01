/**
 * The round viewfinder shown above the composer while a video note records (TG-402): the
 * recorder's own square canvas (already centre-cropped) in a circle, with the minute as a
 * progress ring, over a dimmed, blurred chat (the composer's panel stays clear). The canvas is
 * mounted as-is, so what you see is exactly what is sent.
 */
import { useEffect, useRef } from 'react'
import { VIDEO_NOTE_MAX_MS } from './videoNoteApi'
import { ProgressRing } from './ProgressRing'
import { t } from '../../i18n/index'

export interface VideoNoteViewfinderProps {
  /** The recorder's preview element, or `null` while the camera opens. */
  preview: unknown
  elapsedMs: number
}

export function VideoNoteViewfinder({ preview, elapsedMs }: VideoNoteViewfinderProps) {
  const host = useRef<HTMLDivElement>(null)
  useEffect(() => {
    const node = host.current
    if (!node || typeof Node === 'undefined' || !(preview instanceof Node)) return
    node.appendChild(preview)
    return () => {
      if (preview.parentNode === node) node.removeChild(preview)
    }
  }, [preview])
  return (
    <>
      <div className="tg-video-note-rec__backdrop" aria-hidden="true" />
      <div
        className="tg-video-note-rec"
        role="img"
        aria-label={t('w.videoNote.88d4d7')}
        data-ready={preview ? true : undefined}
      >
        <div className="tg-video-note-rec__lens" ref={host} />
        <ProgressRing className="tg-video-note-rec__ring" progress={elapsedMs / VIDEO_NOTE_MAX_MS} />
      </div>
    </>
  )
}

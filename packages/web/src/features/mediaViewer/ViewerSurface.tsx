/**
 * The open viewer: backdrop, stage (a three-slide track), chrome and thumbnail strip, plus
 * the open/close choreography. Lazy-loaded by `MediaViewer` (it pulls in `motion`).
 * Motion lives in `useViewerMotion`; gestures in `useStageGestures`; this file is layout.
 */
import { useCallback, useEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { Spinner } from '@tg/ui'
import type { MediaItem } from './mediaItem'
import type { MediaViewerRequest } from './mediaViewerStore'
import type { FetchMediaPage } from './mediaPager'
import type { MediaViewerActions } from './types'
import type { Direction } from './mediaNavigation'
import { positionLabel } from './mediaNavigation'
import { useMediaList } from './useMediaList'
import { useStageGestures } from './useStageGestures'
import { useViewerMotion } from './useViewerMotion'
import { findThumbnail, hideThumbnail } from './thumbnailLookup'
import { MediaSlide } from './MediaSlide'
import { Caption, NavArrow, TopBar } from './ViewerChrome'
import { ThumbStrip } from './ThumbStrip'
import { CloseGlyph } from './icons'
import { t } from '../../i18n/index'

export interface ViewerSurfaceProps {
  request: MediaViewerRequest
  actions: MediaViewerActions
  fetchPage: FetchMediaPage
  /** The exit animation finished (or the viewer closed without one). */
  onClosed(): void
}

const OFFSETS = [-1, 0, 1] as const

export default function ViewerSurface({ request, actions, fetchPage, onClosed }: ViewerSurfaceProps) {
  const list = useMediaList(request.chatId, request.attachmentId, fetchPage)
  const mediaRef = useRef<HTMLImageElement>(null)
  const [zoomed, setZoomed] = useState(false)
  const [deleting, setDeleting] = useState(false)
  const [revealed, setRevealed] = useState(() => new Set([request.attachmentId]))
  const current = list.current
  const isRevealed = (item: MediaItem) => !item.isSensitive || revealed.has(item.attachmentId)

  const motion = useViewerMotion({ request, current, mediaRef, onClosed })
  const { refs, phase, flying, reduced, close } = motion

  const step = useCallback(
    (direction: Direction) => {
      gestures.reset()
      return list.go(direction)
    },
    // `gestures` is stable in behaviour (it reads refs); listing it would re-create per render.
    [list.go],
  )

  const gestures = useStageGestures(refs.stage, {
    reduced,
    getMedia: () => (current?.kind === 'image' && isRevealed(current) ? mediaRef.current : null),
    onZoomChange: setZoomed,
    onBackdropTap: close,
    onSwipeMove: (offset) => motion.dragTrack(offset, list.neighbour(offset.dx < 0 ? 1 : -1) !== null),
    onSwipeEnd: (outcome, offset) => {
      if (outcome === 'dismiss') return close()
      const direction: Direction | 0 = outcome === 'next' ? 1 : outcome === 'prev' ? -1 : 0
      const go = direction !== 0 && list.neighbour(direction) !== null
      motion.releaseTrack(offset, go ? direction : 0, () => {
        if (go) step(direction)
      })
    },
  })

  // Keep the current item's bubble thumbnail hidden while its copy is up here.
  const currentUrl = current?.previewUrl ?? ''
  useEffect(() => {
    const thumbnail = findThumbnail(currentUrl)
    return thumbnail ? hideThumbnail(thumbnail.frame) : undefined
  }, [currentUrl])

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault()
        close()
        return
      }
      // A focused player owns its arrow keys (seek).
      if (event.target instanceof Element && event.target.closest('.plyr')) return
      if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') {
        event.preventDefault()
        step(event.key === 'ArrowLeft' ? -1 : 1)
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [close, step])

  // Focus moves into the dialog and returns to the thumbnail's button afterwards. A passive
  // effect declared after the thumbnail-hiding one, so on unmount the thumbnail is visible
  // (hence focusable) again before focus is handed back.
  useEffect(() => {
    const previous = document.activeElement
    refs.root.current?.focus({ preventScroll: true })
    return () => {
      if (previous instanceof HTMLElement) previous.focus({ preventScroll: true })
    }
  }, [refs.root])

  const remove = async () => {
    if (!current || !actions.onDelete || deleting) return
    setDeleting(true)
    try {
      if (!(await actions.onDelete(current))) return
      if (list.removeCurrent() === null) close()
    } finally {
      setDeleting(false)
    }
  }

  const reveal = (id: string) => setRevealed((known) => new Set(known).add(id))
  const complete = !list.hasOlder && !list.syncing

  return createPortal(
    <div
      ref={refs.root}
      className="tg-mv"
      role="dialog"
      aria-modal="true"
      aria-label={t('w.mediaViewer.5bae9e')}
      tabIndex={-1}
      data-phase={phase}
      data-flying={flying ? '' : undefined}
      data-zoomed={zoomed ? '' : undefined}
    >
      <div ref={refs.backdrop} className="tg-mv__backdrop" />
      <div ref={refs.stage} className="tg-mv__stage">
        <div ref={refs.track} className="tg-mv__track">
          {current
            ? OFFSETS.map((offset) => {
                const item = offset === 0 ? current : list.neighbour(offset)
                if (!item) return null
                return (
                  <div
                    key={item.attachmentId}
                    className="tg-mv__slide"
                    data-offset={offset}
                    aria-hidden={offset === 0 ? undefined : 'true'}
                  >
                    <MediaSlide
                      item={item}
                      active={offset === 0}
                      revealed={isRevealed(item)}
                      onReveal={() => reveal(item.attachmentId)}
                      mediaRef={mediaRef}
                      natural={offset === 0 ? motion.naturalOf(item) : null}
                    />
                  </div>
                )
              })
            : null}
        </div>
        {current ? null : (
          <div className="tg-mv__status" data-mv-chrome="">
            {list.syncing ? (
              <Spinner size="lg" label={t('w.mediaViewer.3667cb')} />
            ) : (
              <span>{t('w.mediaViewer.220d4f')}</span>
            )}
          </div>
        )}
      </div>
      {current ? (
        <TopBar
          item={current}
          position={positionLabel(list.index, list.items.length, complete)}
          actions={actions}
          deleting={deleting}
          onForward={() => actions.onForward?.(current)}
          onDelete={() => void remove()}
          onClose={close}
        />
      ) : (
        <button
          type="button"
          className="tg-mv__tool tg-mv__tool--corner"
          aria-label={t('w.mediaViewer.6c14bd')}
          onClick={close}
        >
          <CloseGlyph />
        </button>
      )}
      {list.neighbour(-1) ? <NavArrow direction="left" onClick={() => step(-1)} /> : null}
      {list.neighbour(1) ? <NavArrow direction="right" onClick={() => step(1)} /> : null}
      <footer className="tg-mv__footer">
        {current ? <Caption item={current} /> : null}
        <ThumbStrip
          items={list.items}
          index={list.index}
          reduced={reduced}
          onSelect={(id) => {
            gestures.reset()
            list.select(id)
          }}
        />
      </footer>
      <div ref={refs.flight} className="tg-mv__flight" hidden>
        <img ref={refs.flightImage} alt="" draggable={false} />
      </div>
    </div>,
    document.body,
  )
}

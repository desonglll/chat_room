/**
 * Everything drawn over the stage: the top bar (who/when, position, actions), the side
 * arrows and the caption. Marked `data-mv-chrome` so stage gestures ignore it.
 */
import type { MediaItem } from './mediaItem'
import type { MediaViewerActions } from './types'
import { MessageText } from '../message/content/MessageText'
import { ChevronGlyph, CloseGlyph, DeleteGlyph, DownloadGlyph, ForwardGlyph } from './icons'
import { t } from '../../i18n/index'

const WHEN = new Intl.DateTimeFormat('zh-CN', {
  year: 'numeric',
  month: 'long',
  day: 'numeric',
  hour: '2-digit',
  minute: '2-digit',
})

export function formatWhen(iso: string): string {
  const at = Date.parse(iso)
  return Number.isFinite(at) ? WHEN.format(at) : ''
}

export interface TopBarProps {
  item: MediaItem
  position: string
  actions: MediaViewerActions
  deleting: boolean
  onForward(): void
  onDelete(): void
  onClose(): void
}

export function TopBar({ item, position, actions, deleting, onForward, onDelete, onClose }: TopBarProps) {
  const canDelete = actions.onDelete !== undefined && (actions.canDelete?.(item) ?? true)
  return (
    <header className="tg-mv__bar" data-mv-chrome="">
      <div className="tg-mv__who">
        <span className="tg-mv__sender">{item.sender}</span>
        <span className="tg-mv__when">
          {formatWhen(item.createdAt)}
          {position ? <span className="tg-mv__position">{position}</span> : null}
        </span>
      </div>
      <div className="tg-mv__tools">
        <a
          className="tg-mv__tool"
          href={item.url}
          download={item.fileName}
          aria-label={t('w.mediaViewer.2b9d01')}
          title={t('w.mediaViewer.2b9d01')}
          onClick={
            actions.onDownload
              ? (event) => {
                  event.preventDefault()
                  actions.onDownload?.(item)
                }
              : undefined
          }
        >
          <DownloadGlyph />
        </a>
        {actions.onForward ? (
          <button
            type="button"
            className="tg-mv__tool"
            aria-label={t('w.mediaViewer.0d5a8a')}
            title={t('w.mediaViewer.0d5a8a')}
            onClick={onForward}
          >
            <ForwardGlyph />
          </button>
        ) : null}
        {canDelete ? (
          <button
            type="button"
            className="tg-mv__tool"
            aria-label={t('w.mediaViewer.3755f5')}
            title={t('w.mediaViewer.3755f5')}
            disabled={deleting}
            onClick={onDelete}
          >
            <DeleteGlyph />
          </button>
        ) : null}
        <button
          type="button"
          className="tg-mv__tool"
          aria-label={t('w.mediaViewer.6c14bd')}
          title={t('w.mediaViewer.4417eb')}
          onClick={onClose}
        >
          <CloseGlyph />
        </button>
      </div>
    </header>
  )
}

export function NavArrow({ direction, onClick }: { direction: 'left' | 'right'; onClick(): void }) {
  return (
    <button
      type="button"
      className="tg-mv__arrow"
      data-side={direction}
      data-mv-chrome=""
      aria-label={direction === 'left' ? t('w.mediaViewer.57b271') : t('w.mediaViewer.159a87')}
      onClick={onClick}
    >
      <ChevronGlyph direction={direction} />
    </button>
  )
}

export function Caption({ item }: { item: MediaItem }) {
  if (!item.caption || item.caption.trim() === '') return null
  return (
    <div className="tg-mv__caption" data-mv-chrome="">
      <MessageText text={item.caption} metaSpacer={null} className="tg-mv__caption-text" />
    </div>
  )
}

/**
 * Takes the composer's place while messages are selected (Telegram): count, forward,
 * delete (only when every selected message is the viewer's own), cancel. Escape cancels.
 */
import { useEffect } from 'react'
import { Button, IconButton } from '@tg/ui'
import { CloseGlyph } from '../composer/icons'
import { DeleteGlyph, ForwardGlyph } from '../message/icons'
import { t } from '../../i18n/index'

export interface SelectionBarProps {
  count: number
  canDelete: boolean
  onForward(): void
  onDelete(): void
  onCancel(): void
}

export function SelectionBar({ count, canDelete, onForward, onDelete, onCancel }: SelectionBarProps) {
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !event.defaultPrevented) onCancel()
    }
    document.addEventListener('keydown', onKey)
    return () => document.removeEventListener('keydown', onKey)
  }, [onCancel])

  return (
    <footer className="tg-selection" role="toolbar" aria-label={t('w.chat.f0e0fb')}>
      <IconButton label={t('w.chat.f02e94')} variant="plain" onClick={onCancel}>
        <CloseGlyph size={22} />
      </IconButton>
      <span className="tg-selection__count" aria-live="polite">
        {t('w.chat.f24ddc')} {count} {t('w.chat.bce2ef')}
      </span>
      <Button variant="tonal" onClick={onForward} startIcon={<ForwardGlyph />}>
        {t('w.chat.0d5a8a')}
      </Button>
      <Button variant="danger" onClick={onDelete} disabled={!canDelete} startIcon={<DeleteGlyph />}>
        {t('w.chat.3755f5')}
      </Button>
    </footer>
  )
}

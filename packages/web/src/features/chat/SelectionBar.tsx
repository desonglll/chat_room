/**
 * Takes the composer's place while messages are selected (Telegram): count, forward,
 * delete (only when every selected message is the viewer's own), cancel. Escape cancels.
 */
import { useEffect } from 'react'
import { Button, IconButton } from '@tg/ui'
import { CloseGlyph } from '../composer/icons'
import { DeleteGlyph, ForwardGlyph } from '../message/icons'

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
    <footer className="tg-selection" role="toolbar" aria-label="已选消息">
      <IconButton label="取消选择" variant="plain" onClick={onCancel}>
        <CloseGlyph size={22} />
      </IconButton>
      <span className="tg-selection__count" aria-live="polite">
        已选 {count} 条
      </span>
      <Button variant="tonal" onClick={onForward} startIcon={<ForwardGlyph />}>
        转发
      </Button>
      <Button variant="danger" onClick={onDelete} disabled={!canDelete} startIcon={<DeleteGlyph />}>
        删除
      </Button>
    </footer>
  )
}

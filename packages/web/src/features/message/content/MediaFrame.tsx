import { useState, type ReactNode } from 'react'
import type { Attachment } from '@tg/core'
import type { MessageActions } from '../types'

/**
 * The clickable box around a photo or video. A sensitive attachment is blurred behind a
 * "tap to reveal" veil first; only a revealed (or non-sensitive) medium opens the viewer.
 */
export function MediaFrame({
  attachment,
  actions,
  label,
  children,
}: {
  attachment: Attachment
  actions: MessageActions
  label: string
  children: ReactNode
}) {
  const [revealed, setRevealed] = useState(!attachment.is_sensitive)

  const open = () => {
    if (!revealed) {
      setRevealed(true)
      return
    }
    actions.onOpenMedia?.(attachment.id)
  }

  return (
    <button
      type="button"
      className="tg-bubble__media"
      data-veiled={revealed ? undefined : ''}
      aria-label={revealed ? label : '敏感内容，点击查看'}
      onClick={(event) => {
        event.stopPropagation()
        open()
      }}
    >
      {children}
      {revealed ? null : <span className="tg-bubble__veil">敏感内容 · 点击查看</span>}
    </button>
  )
}

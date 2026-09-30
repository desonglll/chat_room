/**
 * One virtual row: optional date separator, the spacing that separates runs, the jump
 * highlight, and the `renderMessage` call. Spacing is PADDING, never margin — Virtuoso
 * measures border boxes, and a margin would be an unmeasured height that shifts the anchor.
 * Memoized on object identity: the layout cache keeps unchanged entries identical, so an
 * append re-renders only the new row and its predecessor.
 */
import { memo, useMemo } from 'react'
import type { DisplayMessage, MessageLayoutEntry } from '@tg/core'
import type { MessageRenderContext, RenderMessage } from './renderContract'

export interface MessageRowProps {
  message: DisplayMessage
  entry: MessageLayoutEntry
  dayText: string
  highlighted: boolean
  selected: boolean
  renderMessage: RenderMessage
}

function MessageRowImpl({ message, entry, dayText, highlighted, selected, renderMessage }: MessageRowProps) {
  const ctx = useMemo<MessageRenderContext>(
    () => ({
      groupPosition: entry.groupPosition,
      isOutgoing: entry.isOutgoing,
      showAvatar: entry.showAvatar,
      showSenderName: entry.showSenderName,
      highlighted,
      selected,
    }),
    [entry, highlighted, selected],
  )
  return (
    <div
      className="tg-mlist__row"
      data-row-key={entry.key}
      data-joined={entry.joinsPrevious || undefined}
      data-highlighted={highlighted || undefined}
    >
      {entry.startsDay && dayText ? (
        <div className="tg-mlist__date" role="separator">
          <span className="tg-mlist__date-pill">{dayText}</span>
        </div>
      ) : null}
      <div className="tg-mlist__cell">{renderMessage(message, ctx)}</div>
    </div>
  )
}

export const MessageRow = memo(MessageRowImpl)

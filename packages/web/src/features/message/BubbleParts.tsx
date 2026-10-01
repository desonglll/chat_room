/**
 * The decorations above and below a bubble's content: sender name, forwarded-from header,
 * reply quote, reaction chips. Presentational; the only state is a chip's TG-411 burst counter.
 */
import type { ReactNode } from 'react'
import { useState } from 'react'
import type { ForwardedFrom, MessageReaction, ReplyPreview } from '@tg/core'
import { t } from '../../i18n/index'
import { isFreshOwnReaction } from './reactionIntent'

export function SenderName({ name }: { name: string }) {
  return <div className="tg-bubble__sender">{name}</div>
}

export function ForwardHeader({ from }: { from: ForwardedFrom }) {
  const origin = from.room_name.trim()
  return (
    <div className="tg-bubble__forward">
      <span className="tg-bubble__forward-label">{t('w.message.f97f81')}</span>{' '}
      <span className="tg-bubble__forward-name">{from.sender}</span>
      {origin === '' ? null : <span className="tg-bubble__forward-origin"> · {origin}</span>}
    </div>
  )
}

/** What a quote shows for its one line of body text. */
export function replySnippet(reply: ReplyPreview): string {
  if (reply.recalled) return t('w.message.26b4ea')
  const text = reply.content.trim()
  if (text !== '') return text
  if (reply.attachment_file_name) return t('w.message.1a4351', reply.attachment_file_name)
  return t('w.message.dc6de3')
}

export function ReplyQuote({
  reply,
  onJumpTo,
  onOpenSource,
}: {
  reply: ReplyPreview
  onJumpTo?: ((id: string) => void) | undefined
  /** TG-409: a cross-chat reply opens its source chat instead of jumping in this one. */
  onOpenSource?: ((chatId: string, messageId: string) => void) | undefined
}) {
  const crossChat = reply.chat_id !== undefined
  const body = (
    <>
      <span className="tg-bubble__reply-sender">
        {reply.sender}
        {crossChat && reply.chat_title ? <span className="tg-bubble__reply-chat"> · {reply.chat_title}</span> : null}
      </span>
      <span
        className="tg-bubble__reply-text"
        data-recalled={reply.recalled ? '' : undefined}
        data-quote={reply.quote ? '' : undefined}
      >
        {reply.quote && !reply.recalled ? reply.quote.text : replySnippet(reply)}
      </span>
      {reply.quote_modified ? <span className="tg-bubble__reply-modified">{t('w.message.c920e4')}</span> : null}
    </>
  )
  const open =
    crossChat && reply.chat_id !== undefined && onOpenSource
      ? () => onOpenSource(reply.chat_id as string, reply.message_id)
      : crossChat
        ? undefined
        : onJumpTo && (() => onJumpTo(reply.message_id))
  if (open === undefined) return <div className="tg-bubble__reply">{body}</div>
  return (
    <button
      type="button"
      className="tg-bubble__reply"
      aria-label={t('w.message.c4a891', reply.sender)}
      onClick={(event) => {
        event.stopPropagation()
        open()
      }}
    >
      {body}
    </button>
  )
}

export function ReactionRow({
  messageId,
  reactions,
  viewerId,
  onReact,
  metaSpacer,
}: {
  /** Lets a chip mounted by the viewer's own first reaction burst (TG-1201). */
  messageId: string
  reactions: readonly MessageReaction[]
  viewerId: string | undefined
  onReact?: ((emoji: string) => void) | undefined
  metaSpacer: ReactNode
}) {
  return (
    <div className="tg-bubble__reactions">
      {reactions.map((reaction) => (
        <ReactionChip
          key={reaction.emoji}
          emoji={reaction.emoji}
          count={reaction.user_ids.length}
          chosen={viewerId !== undefined && reaction.user_ids.includes(viewerId)}
          fresh={isFreshOwnReaction(messageId, reaction.emoji)}
          onReact={onReact}
        />
      ))}
      {metaSpacer}
    </div>
  )
}

/**
 * One chip. TG-411: when the viewer's own reaction lands (chosen goes false → true while the
 * chip is on screen) the emoji bursts — a pop plus an expanding ring. History never bursts.
 * TG-1201: a chip mounted by the viewer's own first reaction (`fresh`) bursts on mount.
 */
function ReactionChip({
  emoji,
  count,
  chosen,
  fresh,
  onReact,
}: {
  emoji: string
  count: number
  chosen: boolean
  fresh: boolean
  onReact?: ((emoji: string) => void) | undefined
}) {
  const [burst, setBurst] = useState(() => (fresh && chosen ? 1 : 0))
  const [wasChosen, setWasChosen] = useState(chosen)
  if (chosen !== wasChosen) {
    // Adjusting state during render (React's documented pattern) — no effect, no extra frame.
    setWasChosen(chosen)
    if (chosen) setBurst((value) => value + 1)
  }
  return (
    <button
      type="button"
      className="tg-bubble__reaction"
      aria-pressed={chosen}
      aria-label={t('w.message.0cb2c5', emoji, count)}
      disabled={onReact === undefined}
      data-burst={burst > 0 ? '' : undefined}
      onClick={(event) => {
        event.stopPropagation()
        onReact?.(emoji)
      }}
    >
      <span key={burst} className="tg-bubble__reaction-emoji">
        {emoji}
      </span>
      <span className="tg-bubble__reaction-count">{count}</span>
    </button>
  )
}

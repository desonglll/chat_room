/**
 * TG-408: a text message with a link — the text, then its card (when the server built one and
 * it is not hidden), then the timestamp spacer, so the card never sits under the time.
 */
import { useStore } from 'zustand/react'
import { MessageText } from '../message/content/MessageText'
import type { MessageContentProps } from '../message'
import { LinkPreviewCard } from './LinkPreviewCard'
import { effectiveCard, linkPreviewStore } from './linkPreviewStore'

export function LinkPreviewContent({ message, metaSpacer }: MessageContentProps) {
  const held = useStore(linkPreviewStore, (state) => state.cards[message.message_id])
  const card = effectiveCard(message.link_preview, held)
  if (!card) return <MessageText text={message.content} metaSpacer={metaSpacer} />
  return (
    <>
      <MessageText text={message.content} metaSpacer={null} />
      <LinkPreviewCard preview={card} />
      {metaSpacer ? <div className="tg-link-card__meta">{metaSpacer}</div> : null}
    </>
  )
}

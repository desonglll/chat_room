/**
 * Message text with inline custom emoji — the `entity_text` content kind. Same block,
 * link rules and meta spacer as TG-103's `MessageText`; custom emoji ranges become
 * `InlineCustomEmoji`, and copying yields the fallback emoji (see `copyText.ts`).
 */
import type { ReactNode } from 'react'
import { parseMessageEntities, segmentEntityText, type MessageEntity } from '@tg/core'
import type { MessageContentProps } from '../message'
import { linkify } from '../message/content/linkify'
import { handleCustomEmojiCopy } from './copyText'
import { InlineCustomEmoji } from './InlineCustomEmoji'

function LinkedText({ text }: { text: string }) {
  return (
    <>
      {linkify(text).map((segment, index) =>
        segment.type === 'link' ? (
          <a
            key={index}
            className="tg-bubble__link"
            href={segment.href}
            target="_blank"
            rel="noopener noreferrer"
            onClick={(event) => event.stopPropagation()}
          >
            {segment.text}
          </a>
        ) : (
          <span key={index}>{segment.text}</span>
        ),
      )}
    </>
  )
}

export interface EntityTextProps {
  text: string
  entities: readonly MessageEntity[] | unknown
  metaSpacer?: ReactNode
  className?: string | undefined
}

/** Text with inline custom emoji; usable outside bubbles too (previews, replies). */
export function EntityText({ text, entities, metaSpacer = null, className }: EntityTextProps) {
  const segments = segmentEntityText(text, parseMessageEntities(entities, text))
  return (
    <div
      className={`tg-bubble__text tg-entity-text${className ? ` ${className}` : ''}`}
      dir="auto"
      onCopy={handleCustomEmojiCopy}
    >
      {segments.map((segment, index) =>
        segment.kind === 'custom_emoji' ? (
          <InlineCustomEmoji key={index} id={segment.customEmojiId} fallback={segment.text} />
        ) : (
          <LinkedText key={index} text={segment.text} />
        ),
      )}
      {metaSpacer}
    </div>
  )
}

export function EntityTextContent({ message, metaSpacer }: MessageContentProps) {
  return <EntityText text={message.content} entities={message.entities} metaSpacer={metaSpacer} />
}

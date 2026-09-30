import type { ReactNode } from 'react'
import { linkify } from './linkify'

/**
 * A padded text block ending in the meta spacer. Every built-in content that ends in text
 * (plain text, captions, the file card's size line, the deleted placeholder) goes through
 * here, so the no-overlap rule has exactly one implementation.
 */
export function MessageText({
  text,
  metaSpacer,
  className,
}: {
  text: string
  metaSpacer: ReactNode
  className?: string | undefined
}) {
  return (
    <div className={className === undefined ? 'tg-bubble__text' : `tg-bubble__text ${className}`} dir="auto">
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
      {metaSpacer}
    </div>
  )
}

/**
 * Telegram's three-dot activity glyph. Pure CSS: the dots pulse in sequence; under
 * prefers-reduced-motion the scale is dropped and only a slow opacity breath remains
 * (presence.css). Decorative — the adjacent text carries the meaning.
 */
import type { ActiveTypingAction } from '@tg/core'
import './presence.css'

export function TypingIndicator({ action = 'typing' }: { action?: ActiveTypingAction }) {
  return (
    <span className="tg-typing-dots" data-action={action} aria-hidden="true">
      <span className="tg-typing-dots__dot" />
      <span className="tg-typing-dots__dot" />
      <span className="tg-typing-dots__dot" />
    </span>
  )
}

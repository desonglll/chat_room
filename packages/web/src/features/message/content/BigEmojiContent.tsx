/**
 * TG-411: an emoji-only message — no bubble, the emoji drawn large (one is largest). It pops
 * in with the bounce spring when it arrives (row `data-fresh`), and a tap replays a small
 * bounce, Telegram's "poke" effect. Both reduce to a fade under prefers-reduced-motion.
 */
import { useState } from 'react'
import type { BroadcastMessage } from '@tg/core'
import type { MessageContentProps } from './contentTypes'
import { bigEmojiCount } from './bigEmoji'

export function isBigEmojiMessage(message: BroadcastMessage): boolean {
  return (
    message.attachment === null &&
    (message.media_kind ?? '') === '' &&
    (message.entities?.length ?? 0) === 0 &&
    bigEmojiCount(message.content) > 0
  )
}

export function BigEmojiContent({ message, metaSpacer }: MessageContentProps) {
  const [taps, setTaps] = useState(0)
  const count = bigEmojiCount(message.content)
  return (
    <div className="tg-big-emoji" data-count={count}>
      <button
        type="button"
        className="tg-big-emoji__glyphs"
        aria-label={message.content.trim()}
        onClick={(event) => {
          event.stopPropagation()
          setTaps((value) => value + 1)
        }}
      >
        {/* A new key restarts the tap animation on every tap. */}
        <span key={taps} className="tg-big-emoji__text" data-tapped={taps > 0 ? '' : undefined} aria-hidden="true">
          {message.content.trim()}
        </span>
      </button>
      {metaSpacer}
    </div>
  )
}

/**
 * TG-905: a bubble's photo. When the file cannot be decoded the browser would show its broken
 * icon plus the file name; Telegram shows a neutral tile with a photo glyph instead.
 */
import { useState } from 'react'
import { t } from '../../../i18n/index'

/**
 * `fallbackSrc` (TG-1302): the original, tried once if the thumbnail cannot be served (too large
 * or undecodable on the server), before the image is shown as unavailable.
 */
export function BubbleImage({ src, name, fallbackSrc }: { src: string; name: string; fallbackSrc?: string }) {
  const [broken, setBroken] = useState(false)
  const [usingFallback, setUsingFallback] = useState(false)
  const shown = usingFallback && fallbackSrc ? fallbackSrc : src
  if (broken) {
    return (
      <span
        className="tg-bubble__image tg-bubble__image--broken"
        role="img"
        aria-label={t('w.message.imageUnavailable', name)}
      >
        <svg
          width="40"
          height="40"
          viewBox="0 0 24 24"
          aria-hidden="true"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.6"
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          <rect x="3.5" y="5" width="17" height="14" rx="2" />
          <circle cx="9" cy="10" r="1.5" />
          <path d="m4 17 5-4.5 4 3.5 2.5-2 4.5 3.5" />
        </svg>
      </span>
    )
  }
  return (
    <img
      className="tg-bubble__image"
      src={shown}
      alt={name}
      loading="lazy"
      decoding="async"
      draggable={false}
      onError={() => {
        if (!usingFallback && fallbackSrc && fallbackSrc !== src) setUsingFallback(true)
        else setBroken(true)
      }}
    />
  )
}

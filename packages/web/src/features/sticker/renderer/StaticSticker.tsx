/**
 * `<StaticSticker>` — a WebP sticker. The browser decodes it; there is nothing to animate
 * (TG-302 rejects animated WebP), so it never registers with the manager.
 */
import './sticker.css'
import { useState } from 'react'

export interface StaticStickerProps {
  src: string
  size: number
  label?: string | undefined
  className?: string | undefined
  onError?: ((error: unknown) => void) | undefined
}

export function StaticSticker({ src, size, label, className, onError }: StaticStickerProps) {
  const [loaded, setLoaded] = useState<{ src: string; ok: boolean } | null>(null)
  const phase = loaded?.src !== src ? 'loading' : loaded.ok ? 'static' : 'error'
  return (
    <span
      className={className ? `tg-sticker ${className}` : 'tg-sticker'}
      data-phase={phase}
      data-format="webp"
      role="img"
      aria-label={label}
      style={{ width: size, height: size }}
    >
      <img
        key={src}
        className="tg-sticker__still"
        src={src}
        alt=""
        draggable={false}
        decoding="async"
        hidden={phase === 'error'}
        onLoad={() => setLoaded({ src, ok: true })}
        onError={() => {
          setLoaded({ src, ok: false })
          onError?.(new Error('webp sticker failed to load'))
        }}
      />
    </span>
  )
}

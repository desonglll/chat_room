/**
 * The enlarged sticker shown while a press is held: a centred, pointer-transparent layer
 * over the whole viewport with the sticker at preview size and its emoji underneath.
 * It scales in with the spring token; reduced motion gets a plain fade (sticker.css).
 */
import { createPortal } from 'react-dom'
import type { Sticker } from '@tg/core'
import { StickerView } from '../StickerView'

export const PREVIEW_SIZE = 256

export function StickerPreview({ sticker }: { sticker: Sticker | null }) {
  if (sticker === null || typeof document === 'undefined') return null
  return createPortal(
    <div className="tg-sticker-preview" role="dialog" aria-label={`贴纸预览 ${sticker.emoji}`}>
      <div className="tg-sticker-preview__card" key={sticker.id}>
        <StickerView src={sticker.file_url} format={sticker.format} size={PREVIEW_SIZE} label={sticker.emoji} />
        <span className="tg-sticker-preview__emoji">{sticker.emojis.join(' ') || sticker.emoji}</span>
      </div>
    </div>,
    document.body,
  )
}

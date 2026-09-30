/**
 * The body of a sticker message: the sticker itself with no bubble (the registry's `bare`
 * frame) and the time as an overlay pill, as in Telegram. Clicking it opens its pack.
 */
import type { MessageContentProps } from '../../message'
import { openStickerSet } from '../overlayStore'
import { StickerView } from '../StickerView'
import { stickerMessageView } from './stickerMessageModel'

export function StickerMessage({ message }: MessageContentProps) {
  const view = stickerMessageView(message)
  if (view === null) return null
  const label = view.emoji ? `贴纸 ${view.emoji}` : '贴纸'
  const setShortName = view.setShortName
  const body = (
    <StickerView
      src={view.src}
      format={view.format}
      size={Math.max(view.width, view.height)}
      label={label}
      className="tg-sticker-message__art"
    />
  )
  return (
    <div className="tg-sticker-message" style={{ width: view.width, height: view.height }}>
      {setShortName ? (
        <button
          type="button"
          className="tg-sticker-message__open"
          aria-label={`${label}，查看贴纸包`}
          onClick={(event) => {
            event.stopPropagation()
            openStickerSet(setShortName)
          }}
        >
          {body}
        </button>
      ) : (
        body
      )}
    </div>
  )
}

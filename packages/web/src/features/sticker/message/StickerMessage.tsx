/**
 * The body of a sticker message: the sticker itself with no bubble (the registry's `bare`
 * frame) and the time as an overlay pill, as in Telegram. Clicking it opens its pack.
 */
import type { MessageContentProps } from '../../message'
import { openStickerSet } from '../overlayStore'
import { LazyStickerView as StickerView } from '../LazyStickerView'
import { stickerMessageView } from './stickerMessageModel'
import { t } from '../../../i18n/index'

export function StickerMessage({ message }: MessageContentProps) {
  const view = stickerMessageView(message)
  if (view === null) return null
  const label = view.emoji ? t('w.sticker.3c8532', view.emoji) : t('w.sticker.f7c0f3')
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
          aria-label={t('w.sticker.cc459f', label)}
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

/**
 * The strip of sticker suggestions that floats above the input while the draft is exactly
 * one emoji (Telegram's "emoji → sticker" hint). Picking one sends it and clears the draft;
 * Esc or ✕ hides the strip until the draft changes.
 *
 * The composer mounts this with the live draft; everything else (library load, lookup,
 * timing) lives here and in `suggestionController.ts`.
 */
import { useEffect, useMemo, useState } from 'react'
import { stickerStore, suggestStickers, type Sticker } from '@tg/core'
import { useStore } from 'zustand/react'
import { browserClock } from '../../../app/platform'
import { StickerView } from '../StickerView'
import { stickerLibrary } from '../stickerLibrary'
import { createSuggestionController, NO_SUGGESTIONS, type Suggestions } from './suggestionController'
import '../sticker.css'

export interface StickerSuggestionsProps {
  draft: string
  onPick(sticker: Sticker): void
  /** Disabled while the chat cannot send (offline, read-only). */
  disabled?: boolean | undefined
}

const THUMB = 64

export function StickerSuggestions({ draft, onPick, disabled = false }: StickerSuggestionsProps) {
  const [shown, setShown] = useState<Suggestions>(NO_SUGGESTIONS)
  const controller = useMemo(
    () =>
      createSuggestionController({
        clock: browserClock,
        lookup: (emoji) => suggestStickers(stickerStore.getState(), emoji),
        onChange: setShown,
      }),
    [],
  )
  const library = useStore(stickerStore, (state) => state.sets)
  const recent = useStore(stickerStore, (state) => state.recent)
  const favorites = useStore(stickerStore, (state) => state.favorites)

  useEffect(() => () => controller.dispose(), [controller])
  useEffect(() => controller.update(draft), [controller, draft])
  useEffect(() => controller.refresh(), [controller, library, recent, favorites])
  // The first emoji typed is what makes the library worth loading.
  const wantsLibrary = shown.emoji !== '' || /\p{Extended_Pictographic}/u.test(draft)
  useEffect(() => {
    if (wantsLibrary)
      void stickerLibrary()
        .ensureLoaded()
        .catch(() => undefined)
  }, [wantsLibrary])

  useEffect(() => {
    if (shown.stickers.length === 0) return
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return
      event.stopPropagation()
      controller.dismiss()
    }
    window.addEventListener('keydown', onKey, true)
    return () => window.removeEventListener('keydown', onKey, true)
  }, [controller, shown])

  if (shown.stickers.length === 0) return null
  return (
    <div className="tg-sticker-suggest" role="listbox" aria-label={`${shown.emoji} 的贴纸`} data-emoji={shown.emoji}>
      {shown.stickers.map((sticker) => (
        <button
          key={sticker.id}
          type="button"
          role="option"
          aria-selected={false}
          className="tg-sticker-suggest__item"
          disabled={disabled}
          aria-label={`发送贴纸 ${sticker.emoji}`}
          onMouseDown={(event) => event.preventDefault()}
          onClick={() => onPick(sticker)}
        >
          <StickerView src={sticker.file_url} format={sticker.format} size={THUMB} label={sticker.emoji} />
        </button>
      ))}
    </div>
  )
}

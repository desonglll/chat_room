/**
 * The grid of one or more custom emoji sets — the body of the picker tab and of the emoji
 * status picker. Pure: sets in, `onPick` out.
 */
import type { CustomEmojiSet } from '@tg/core'
import { InlineCustomEmoji } from './InlineCustomEmoji'

export interface PickedCustomEmoji {
  id: string
  /** The fallback Unicode emoji, inserted into the text. */
  emoji: string
}

export interface CustomEmojiGridProps {
  sets: readonly CustomEmojiSet[]
  onPick(emoji: PickedCustomEmoji): void
}

export function CustomEmojiGrid({ sets, onPick }: CustomEmojiGridProps) {
  const visible = sets.filter((set) => !set.archived && set.stickers.length > 0)
  if (visible.length === 0) {
    return <p className="tg-custom-emoji-grid__empty">还没有添加自定义表情包</p>
  }
  return (
    <div className="tg-custom-emoji-grid">
      {visible.map((set) => (
        <section key={set.id} className="tg-custom-emoji-grid__set" aria-label={set.title}>
          <h3 className="tg-custom-emoji-grid__title">{set.title}</h3>
          <div className="tg-custom-emoji-grid__cells">
            {set.stickers.map((sticker) => (
              <button
                key={sticker.id}
                type="button"
                className="tg-custom-emoji-grid__cell"
                title={sticker.emoji}
                onClick={() => onPick({ id: sticker.id, emoji: sticker.emoji })}
              >
                <InlineCustomEmoji id={sticker.id} fallback={sticker.emoji} size={32} />
              </button>
            ))}
          </div>
        </section>
      ))}
    </div>
  )
}

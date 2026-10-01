/**
 * TG-901: the row of quick reactions at the top of a message's right-click / long-press
 * menu, as in Telegram — one click reacts and closes the menu. The same emoji set and
 * styling as the hover bar's picker.
 */
import { QUICK_REACTIONS } from '@tg/core'
import { t } from '../../i18n/index'

export function QuickReactionStrip({ onPick }: { onPick: (emoji: string) => void }) {
  return (
    <div
      className="tg-bubble__reaction-picker tg-message__menu-reactions"
      role="group"
      aria-label={t('w.message.57e26d')}
    >
      {QUICK_REACTIONS.map((emoji) => (
        <button
          key={emoji}
          type="button"
          className="tg-bubble__reaction-pick"
          aria-label={t('w.message.0929b9', emoji)}
          onClick={() => onPick(emoji)}
        >
          {emoji}
        </button>
      ))}
    </div>
  )
}

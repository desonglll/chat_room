/** TG-207: the strip above the composer while the viewer must wait (Telegram: «慢速模式»). */
import { slowModeCountdown } from '@tg/core'
import { t } from '../../../i18n/index'

export function SlowModeNotice({ wait }: { wait: number }) {
  if (wait <= 0) return null
  return (
    <div className="tg-slowmode-notice" role="status" aria-live="polite">
      {t('w.chatAdmin.e69d6d')} <span className="tg-slowmode-notice__time">{slowModeCountdown(wait)}</span>
    </div>
  )
}

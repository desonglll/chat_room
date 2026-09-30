/** TG-207: the strip above the composer while the viewer must wait (Telegram: «慢速模式»). */
import { slowModeCountdown } from '@tg/core'

export function SlowModeNotice({ wait }: { wait: number }) {
  if (wait <= 0) return null
  return (
    <div className="tg-slowmode-notice" role="status" aria-live="polite">
      慢速模式已开启 · 还需等待 <span className="tg-slowmode-notice__time">{slowModeCountdown(wait)}</span>
    </div>
  )
}

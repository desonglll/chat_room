import { t } from '../../i18n/index'
/**
 * Date-separator copy for a local day number from `localDayKey` (days since the epoch in
 * local wall-clock time). Telegram's rules: 今天 / 昨天 / "9月30日" this year / "2025年9月30日".
 */
const DAY_MS = 86_400_000

const monthDay = new Intl.DateTimeFormat('zh-CN', { month: 'long', day: 'numeric', timeZone: 'UTC' })
const fullDate = new Intl.DateTimeFormat('zh-CN', { year: 'numeric', month: 'long', day: 'numeric', timeZone: 'UTC' })

export function dayLabel(day: number, today: number): string {
  if (Number.isNaN(day)) return ''
  if (day === today) return t('w.messageList.17e83c')
  if (day === today - 1) return t('w.messageList.59c4fc')
  // `day * DAY_MS` is the UTC instant whose UTC calendar fields ARE the local date.
  const instant = new Date(day * DAY_MS)
  const sameYear = instant.getUTCFullYear() === new Date(today * DAY_MS).getUTCFullYear()
  return (sameYear ? monthDay : fullDate).format(instant)
}

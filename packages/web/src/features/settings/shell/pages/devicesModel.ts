/**
 * 设备 page logic (TG-110), pure: order the sessions Telegram's way (this device first, then
 * most recently active) and describe when each was last used.
 */
import type { DeviceSession } from '@tg/core'
import { t } from '../../../../i18n/index'

export function orderDeviceSessions(sessions: readonly DeviceSession[]): {
  current: DeviceSession | undefined
  others: DeviceSession[]
} {
  const current = sessions.find((session) => session.current)
  const others = sessions
    .filter((session) => !session.current)
    .sort((a, b) => Date.parse(b.last_used_at) - Date.parse(a.last_used_at))
  return { current, others }
}

const pad = (value: number) => String(value).padStart(2, '0')

/** «刚刚» within a minute, «HH:MM» today, «M月D日» this year, else «YYYY/M/D». */
export function describeLastActive(iso: string, now: Date): string {
  const at = new Date(iso)
  if (Number.isNaN(at.getTime())) return ''
  if (now.getTime() - at.getTime() < 60_000) return t('w.settings.9e6366')
  if (at.toDateString() === now.toDateString()) return `${pad(at.getHours())}:${pad(at.getMinutes())}`
  if (at.getFullYear() === now.getFullYear()) return t('w.settings.fa0b40', at.getMonth() + 1, at.getDate())
  return `${at.getFullYear()}/${at.getMonth() + 1}/${at.getDate()}`
}

export function deviceSubtitle(session: DeviceSession, now: Date): string {
  const parts = [
    session.ip_hint ?? '',
    session.current ? t('w.settings.0373ff') : describeLastActive(session.last_used_at, now),
  ]
  return parts.filter((part) => part !== '').join(' · ')
}

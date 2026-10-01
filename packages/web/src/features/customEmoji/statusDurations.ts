import { t } from '../../i18n/index'
/** The expiry choices of the emoji status picker (Telegram's set). `null` = until cleared. */
export interface StatusDuration {
  label: string
  ms: number | null
}

export const STATUS_DURATIONS: readonly StatusDuration[] = [
  {
    get label() {
      return t('w.customEmoji.409752')
    },
    ms: null,
  },
  {
    get label() {
      return t('w.customEmoji.c8fb1c')
    },
    ms: 3_600_000,
  },
  {
    get label() {
      return t('w.customEmoji.759ce5')
    },
    ms: 8 * 3_600_000,
  },
  {
    get label() {
      return t('w.customEmoji.11f478')
    },
    ms: 24 * 3_600_000,
  },
  {
    get label() {
      return t('w.customEmoji.03f320')
    },
    ms: 7 * 24 * 3_600_000,
  },
]

export function expiryFor(duration: StatusDuration, now: number): string | null {
  return duration.ms === null ? null : new Date(now + duration.ms).toISOString()
}

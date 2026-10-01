/**
 * Human copy for the second login stage (TG-506). Statuses map to Chinese here, at the
 * feature boundary, exactly like `authCopy.ts`; server strings are never matched.
 */
import { ApiError } from '@tg/core'
import { t } from '../../i18n/index'

export type SecondStageAction = 'password' | 'recovery-request' | 'recovery-code'

/** True when the pending token is spent or expired and the user must start over. */
export function isChallengeGone(error: unknown): boolean {
  return error instanceof ApiError && error.status === 410
}

export function secondStageErrorCopy(error: unknown, action: SecondStageAction): string {
  if (!(error instanceof ApiError)) return t('w.auth.77d57e')
  switch (error.status) {
    case 400:
      return action === 'recovery-code' ? t('w.auth.d0fa3c') : t('w.auth.1bb472')
    case 401:
      return action === 'recovery-code' ? t('w.auth.751a0d') : t('w.auth.8c7930')
    case 409:
      return t('w.auth.7f0c4e')
    case 410:
      return t('w.auth.0b996a')
    case 429:
      return t('w.auth.b04f0b')
    case 502:
    case 503:
      return t('w.auth.afc70c')
    default:
      return t('w.auth.d8ae3d', error.status)
  }
}

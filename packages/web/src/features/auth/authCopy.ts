/**
 * Human copy for the auth flow. `ApiError` deliberately carries no UI copy
 * (TG-011 decision); statuses map to Chinese here, at the feature boundary, and
 * server `error` strings are never string-matched so backend copy can change freely.
 */
import { ApiError } from '@tg/core'
import type { AuthMode } from '../../app/session'
import { t } from '../../i18n/index'

export function authErrorCopy(error: unknown, mode: AuthMode): string {
  if (error instanceof ApiError) {
    switch (error.status) {
      case 400:
        return t('w.auth.a8b77c')
      case 401:
        return t('w.auth.04a4a7')
      case 403:
        return mode === 'register' ? t('w.auth.da9028') : t('w.auth.428d1f')
      case 409:
        return t('w.auth.2202dd')
      case 429:
        return t('w.auth.b04f0b')
      default:
        return t('w.auth.d8ae3d', error.status)
    }
  }
  return t('w.auth.77d57e')
}

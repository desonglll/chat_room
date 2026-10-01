/**
 * TG-1301: why the device position cannot be read, as user-facing copy. Outside a secure context
 * (http on a LAN address) browsers keep `navigator.geolocation` but answer every request with
 * PERMISSION_DENIED, which used to read as «未获得定位权限» — advice the user could not act on.
 */
import { t } from '../../i18n/index'

interface LocationScope {
  isSecureContext?: boolean
  navigator?: { geolocation?: unknown }
}

/** Copy for a position that cannot even be asked for, or `null` when asking may work. */
export function locationBlockedReason(scope: LocationScope = globalThis): string | null {
  if (scope.isSecureContext === false) return t('w.location.a14d34')
  if (!scope.navigator?.geolocation) return t('w.location.2332ee')
  return null
}

/** Copy for a failed `getCurrentPosition` (code 1 = PERMISSION_DENIED). */
export function locationFailureReason(failure: { code: number }): string {
  return failure.code === 1 ? t('w.location.db9cc6') : t('w.location.96fe75')
}

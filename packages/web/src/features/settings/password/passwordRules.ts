/** TG-704: the new-password rule, mirroring the server (8–256 characters). */

export const PASSWORD_MIN = 8
export const PASSWORD_MAX = 256

/** Why a new password cannot be submitted yet, as an i18n key; '' when it can. */
export function newPasswordProblem(current: string, next: string, repeat: string): string {
  if (!current) return 'w.password.currentRequired'
  const length = [...next].length
  if (length < PASSWORD_MIN) return 'w.password.tooShort'
  if (length > PASSWORD_MAX) return 'w.password.tooLong'
  if (next !== repeat) return 'w.password.mismatch'
  if (next === current) return 'w.password.same'
  return ''
}

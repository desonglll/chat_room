/**
 * Pure rules and copy for the cloud-password settings (TG-506). The server enforces the
 * same rules; checking them here only saves a round trip and gives a precise message.
 */
import { ApiError } from '@tg/core'
import { t } from '../../../i18n/index'

export const MAX_HINT_CHARS = 64

export type SecurityView = 'overview' | 'enable' | 'change' | 'email' | 'disable'

export interface PasswordDraft {
  password: string
  confirm: string
  hint: string
}

/** A message for the first problem, or '' when the draft may be submitted. */
export function checkNewPassword({ password, confirm, hint }: PasswordDraft): string {
  if (password.length === 0) return t('w.settings.1bb472')
  if ([...password].length > 256) return t('w.settings.66c3b2')
  if (password !== confirm) return t('w.settings.3e2b22')
  return checkHint(hint, password)
}

export function checkHint(hint: string, password: string): string {
  const trimmed = hint.trim()
  if ([...trimmed].length > MAX_HINT_CHARS) return t('w.settings.e027e3', MAX_HINT_CHARS)
  if (trimmed && password && trimmed.toLowerCase().includes(password.toLowerCase())) return t('w.settings.e0e84b')
  return ''
}

export function checkEmail(email: string): string {
  const value = email.trim()
  const at = value.indexOf('@')
  const domain = value.slice(at + 1)
  const valid =
    at > 0 &&
    value.indexOf('@', at + 1) === -1 &&
    domain.includes('.') &&
    !domain.startsWith('.') &&
    !domain.endsWith('.') &&
    !/\s/.test(value)
  return valid ? '' : t('w.settings.25d22e')
}

export function checkCode(code: string): string {
  return /^\d{6}$/.test(code.trim()) ? '' : t('w.settings.d0fa3c')
}

export type SecurityAction = 'load' | 'enable' | 'change' | 'disable' | 'email' | 'code'

export function securityErrorCopy(error: unknown, action: SecurityAction): string {
  if (!(error instanceof ApiError)) return t('w.settings.77d57e')
  switch (error.status) {
    case 400:
      return action === 'code' ? t('w.settings.d0fa3c') : t('w.settings.06ff91')
    case 401:
      if (action === 'enable') return t('w.settings.7b2330')
      if (action === 'code') return t('w.settings.751a0d')
      if (action === 'load') return t('w.settings.dc29c0')
      return t('w.settings.8c7930')
    case 404:
      return t('w.settings.a1a1e2')
    case 409:
      return t('w.settings.985220')
    case 410:
      return t('w.settings.a3bf92')
    case 429:
      return t('w.settings.b04f0b')
    case 502:
    case 503:
      return t('w.settings.bba9a7')
    default:
      return t('w.settings.d8ae3d', error.status)
  }
}

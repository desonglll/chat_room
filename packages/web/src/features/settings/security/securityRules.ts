/**
 * Pure rules and copy for the cloud-password settings (TG-506). The server enforces the
 * same rules; checking them here only saves a round trip and gives a precise message.
 */
import { ApiError } from '@tg/core'

export const MAX_HINT_CHARS = 64

export type SecurityView = 'overview' | 'enable' | 'change' | 'email' | 'disable'

export interface PasswordDraft {
  password: string
  confirm: string
  hint: string
}

/** A message for the first problem, or '' when the draft may be submitted. */
export function checkNewPassword({ password, confirm, hint }: PasswordDraft): string {
  if (password.length === 0) return '请输入两步验证密码'
  if ([...password].length > 256) return '密码不能超过 256 个字符'
  if (password !== confirm) return '两次输入的密码不一致'
  return checkHint(hint, password)
}

export function checkHint(hint: string, password: string): string {
  const trimmed = hint.trim()
  if ([...trimmed].length > MAX_HINT_CHARS) return `提示不能超过 ${MAX_HINT_CHARS} 个字符`
  if (trimmed && password && trimmed.toLowerCase().includes(password.toLowerCase())) return '提示中不能包含密码本身'
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
  return valid ? '' : '请输入有效的邮箱地址'
}

export function checkCode(code: string): string {
  return /^\d{6}$/.test(code.trim()) ? '' : '验证码是 6 位数字'
}

export type SecurityAction = 'load' | 'enable' | 'change' | 'disable' | 'email' | 'code'

export function securityErrorCopy(error: unknown, action: SecurityAction): string {
  if (!(error instanceof ApiError)) return '无法连接服务器，请检查网络后重试'
  switch (error.status) {
    case 400:
      return action === 'code' ? '验证码是 6 位数字' : '输入不符合要求，请检查后重试'
    case 401:
      if (action === 'enable') return '账号密码不正确'
      if (action === 'code') return '验证码不正确'
      if (action === 'load') return '登录已失效，请重新登录'
      return '两步验证密码不正确'
    case 404:
      return '两步验证尚未开启'
    case 409:
      return '两步验证已经开启'
    case 410:
      return '验证码已失效，请重新发送'
    case 429:
      return '尝试次数过多，请稍后再试'
    case 502:
    case 503:
      return '服务器未配置邮件发送，暂时无法设置恢复邮箱'
    default:
      return `请求失败（${error.status}），请稍后再试`
  }
}

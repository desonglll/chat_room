/**
 * Human copy for the second login stage (TG-506). Statuses map to Chinese here, at the
 * feature boundary, exactly like `authCopy.ts`; server strings are never matched.
 */
import { ApiError } from '@tg/core'

export type SecondStageAction = 'password' | 'recovery-request' | 'recovery-code'

/** True when the pending token is spent or expired and the user must start over. */
export function isChallengeGone(error: unknown): boolean {
  return error instanceof ApiError && error.status === 410
}

export function secondStageErrorCopy(error: unknown, action: SecondStageAction): string {
  if (!(error instanceof ApiError)) return '无法连接服务器，请检查网络后重试'
  switch (error.status) {
    case 400:
      return action === 'recovery-code' ? '验证码是 6 位数字' : '请输入两步验证密码'
    case 401:
      return action === 'recovery-code' ? '验证码不正确' : '两步验证密码不正确'
    case 409:
      return '这个账号没有设置恢复邮箱，无法通过邮件重置'
    case 410:
      return '登录已超时，请重新输入账号密码'
    case 429:
      return '尝试次数过多，请稍后再试'
    case 502:
    case 503:
      return '服务器暂时无法发送邮件，请联系管理员'
    default:
      return `请求失败（${error.status}），请稍后再试`
  }
}
